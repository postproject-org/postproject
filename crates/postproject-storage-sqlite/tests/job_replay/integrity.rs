use postproject_protocol::{Document, Limits};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

use super::{fixture, support};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table checks whole-publication rollback for each contradiction"
)]
fn rehashed_job_contradictions_rollback_all_lifecycle_and_publication_facts() {
    let directory = tempfile::tempdir().unwrap();
    let (source, jobs, _) = fixture(directory.path());
    let original = source.record_reader(2).unwrap().manifest().clone();
    let frames = support::frames(&source, 2);
    let cases: [(&str, usize, &[&str], serde_json::Value); 8] = [
        (
            "job.transition",
            0,
            &["previous", "kind"],
            "cancelled".into(),
        ),
        ("job.transition", 0, &["input_boundary"], "3".into()),
        (
            "job.transition",
            0,
            &["state", "claim_id"],
            jobs[0].to_string().into(),
        ),
        (
            "job.transition",
            1,
            &["state", "tool", "name"],
            "different".into(),
        ),
        (
            "job.transition",
            2,
            &["authority_time_micros"],
            i64::MAX.to_string().into(),
        ),
        ("job.transition", 4, &["state", "diagnostic"], "".into()),
        (
            "job.transition",
            8,
            &["state", "representation_id"],
            jobs[0].to_string().into(),
        ),
        ("job.transition", 8, &["input_boundary"], "3".into()),
    ];
    for (case, (kind, index, fields, value)) in cases.into_iter().enumerate() {
        let mut altered = frames.clone();
        let frame = altered
            .iter_mut()
            .filter(|frame| frame.kind().unwrap() == kind)
            .nth(index)
            .unwrap();
        let mut object: serde_json::Value =
            serde_json::from_slice(&frame.canonical_bytes().unwrap()).unwrap();
        let mut selected = &mut object;
        for field in fields {
            selected = &mut selected[*field];
        }
        *selected = value;
        *frame = Document::parse(&serde_json::to_vec(&object).unwrap(), Limits::default()).unwrap();
        let (manifest, chunk) = support::rehashed(&original, &altered);
        let path = directory.path().join(format!("rejected-{case}.pproj"));
        let mut mirror = SqliteProduction::create_genesis_mirror(
            &path,
            source.production(),
            source.exchange_floor().unwrap(),
        )
        .unwrap();
        support::apply(&source, &mut mirror, 1);
        assert!(
            mirror
                .apply_record(&manifest, [Ok(chunk)], ReplayLimits::default())
                .is_err(),
            "case {case}"
        );
        for job in jobs {
            assert!(matches!(
                mirror.job(job).unwrap().state(),
                postproject_core::JobState::Requested
            ));
        }
        assert_eq!(mirror.exchange_head().unwrap().sequence(), 1);
        assert_eq!(
            mirror.activities().unwrap(),
            Vec::<postproject_core::Activity>::new()
        );
        drop(mirror);
        let connection = rusqlite::Connection::open(path).unwrap();
        for table in [
            "activity_inputs",
            "activity_outputs",
            "activity_output_keys",
            "representation_fingerprint_history",
        ] {
            assert_eq!(
                connection
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0,
                "case {case}: {table}"
            );
        }
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM representations", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
