use std::path::Path;
use std::time::Duration;

use postproject_core::{JobId, ToolIdentity};
use postproject_storage_sqlite::{CheckpointLimits, SqliteProduction};

use super::{fixture, request, support};

fn checkpoint(source: &SqliteProduction, path: &Path, secret: &str) -> SqliteProduction {
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            assert!(!String::from_utf8_lossy(chunk.payload()).contains(secret));
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    SqliteProduction::import_checkpoint(
        path,
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap()
}

fn compare(source: &SqliteProduction, mirror: &SqliteProduction, jobs: &[JobId]) {
    for id in jobs {
        assert_eq!(source.job(*id).unwrap(), mirror.job(*id).unwrap());
    }
    assert_eq!(source.activities().unwrap(), mirror.activities().unwrap());
    assert_eq!(
        source.exchange_head().unwrap(),
        mirror.exchange_head().unwrap()
    );
    let revisions = source.changes_since(0, 10).unwrap();
    assert_eq!(revisions, mirror.changes_since(0, 10).unwrap());
    for revision in revisions {
        assert_eq!(
            source.events_for_revision(revision.id()).unwrap(),
            mirror.events_for_revision(revision.id()).unwrap()
        );
    }
}

#[test]
fn checkpoint_and_suffix_match_genesis_replay_without_worker_authority() {
    let directory = tempfile::tempdir().unwrap();
    let (mut source, ids, token) = fixture(directory.path());
    let early_path = directory.path().join("early.pproj");
    let mut early = checkpoint(&source, &early_path, &token);
    let mut replay = SqliteProduction::create_genesis_mirror(
        directory.path().join("replay.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    for sequence in 1..=2 {
        support::apply(&source, &mut replay, sequence);
    }
    for mirror in [&mut early, &mut replay] {
        compare(&source, mirror, &ids);
        assert!(mirror.import_job_lease(&token).is_err());
        assert!(mirror.begin_transaction().is_err());
    }
    let media = support::media(1, true);
    let pending = request(media.asset().id(), media.representation().id());
    let lease = source.import_job_lease(&token).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.release_job_lease(&lease).unwrap();
    edit.request_job(&pending).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let jobs = [ids[0], ids[1], ids[2], ids[3], pending.id()];
    let late_path = directory.path().join("late.pproj");
    let mut late = checkpoint(&source, &late_path, &token);
    for mirror in [&mut early, &mut replay] {
        support::apply(&source, mirror, 3);
    }
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    let lease = edit
        .claim_job_lease(
            ids[3],
            &ToolIdentity::new(" Other 名 ", None, None).unwrap(),
            None,
            Duration::from_secs(3600),
        )
        .unwrap();
    edit.cancel_job(pending.id()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let current_token = lease.export_token().unwrap();
    for mirror in [&mut early, &mut late, &mut replay] {
        support::apply(&source, mirror, 4);
        compare(&source, mirror, &jobs);
        assert!(mirror.import_job_lease(&current_token).is_err());
    }
    drop(early);
    drop(late);
    for path in [early_path, late_path] {
        let mut reopened = SqliteProduction::open(&path).unwrap();
        compare(&source, &reopened, &jobs);
        assert!(reopened.import_job_lease(&current_token).is_err());
        let connection = rusqlite::Connection::open(path).unwrap();
        let private: (i64, i64) = connection
            .query_row(
                "SELECT COUNT(claim_id), SUM(claim_inert) FROM jobs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(private, (0, 1));
        assert_eq!(
            connection
                .query_row("SELECT high_water_micros FROM job_clock", [], |row| row
                    .get::<_, Option<
                    i64,
                >>(
                    0
                ))
                .unwrap(),
            None
        );
    }
}
