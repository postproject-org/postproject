use postproject_protocol::{Document, Limits, RecordChunk, RecordManifest};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

use super::{source, support};

fn rejected(
    source: &SqliteProduction,
    path: &std::path::Path,
    manifest: &RecordManifest,
    chunk: RecordChunk,
) {
    let mut mirror = SqliteProduction::create_genesis_mirror(
        path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    assert!(
        mirror
            .apply_record(manifest, [Ok(chunk)], ReplayLimits::default())
            .is_err()
    );
    assert_eq!(mirror.exchange_head().unwrap().sequence(), 0);
    drop(mirror);
    let connection = rusqlite::Connection::open(path).unwrap();
    for table in [
        "assets",
        "representations",
        "resources",
        "activities",
        "activity_inputs",
        "activity_outputs",
        "activity_output_keys",
        "activity_input_fingerprint_snapshots",
        "activity_output_fingerprint_snapshots",
        "activity_input_dependency_snapshots",
        "activity_input_dependency_paths",
        "activity_input_dependency_path_edges",
        "activity_input_dependency_fingerprint_snapshots",
        "revisions",
        "revision_events",
        "conflict_versions",
        "exchange_records",
        "exchange_record_chunks",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "published a prefix in {table}");
    }
}

#[test]
fn rehashed_activity_contradictions_leave_no_publication_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let (source, _) = source(directory.path());
    let manifest = source.record_reader(1).unwrap().manifest().clone();
    let documents = support::frames(&source, 1);
    let foreign = postproject_core::RepresentationId::new().to_string();
    let cases: [(&str, usize, &[&str], serde_json::Value); 12] = [
        ("activity.header", 0, &["input_count"], "2".into()),
        ("activity.header", 0, &["output_count"], "3".into()),
        (
            "activity.edge",
            0,
            &["representation_id"],
            foreign.clone().into(),
        ),
        (
            "activity.edge",
            0,
            &["snapshot_revision_sequence"],
            "2".into(),
        ),
        ("activity.edge", 0, &["position"], "1".into()),
        ("activity.edge", 0, &["side"], "output".into()),
        ("activity.edge", 0, &["dependency_snapshot"], false.into()),
        (
            "activity.dependency-path",
            0,
            &["subject_representation_id"],
            foreign.clone().into(),
        ),
        (
            "activity.dependency-path",
            0,
            &["status"],
            "representations-truncated".into(),
        ),
        (
            "activity.dependency-path",
            0,
            &["fingerprint_count"],
            "0".into(),
        ),
        (
            "activity.dependency-segment",
            0,
            &["occurrence", "position"],
            "1".into(),
        ),
        (
            "activity.dependency-segment",
            0,
            &["occurrence", "dependency", "authored_reference"],
            "changed".into(),
        ),
    ];
    for (index, (kind, ordinal, fields, value)) in cases.into_iter().enumerate() {
        let mut changed = documents.clone();
        let document = changed
            .iter_mut()
            .filter(|document| document.kind().unwrap() == kind)
            .nth(ordinal)
            .unwrap();
        let mut wire: serde_json::Value =
            serde_json::from_slice(&document.canonical_bytes().unwrap()).unwrap();
        let mut selected = &mut wire;
        for field in fields {
            selected = &mut selected[*field];
        }
        *selected = value;
        *document =
            Document::parse(&serde_json::to_vec(&wire).unwrap(), Limits::default()).unwrap();
        let (manifest, chunk) = support::rehashed(&manifest, &changed);
        rejected(
            &source,
            &directory.path().join(format!("forged-{index}.pproj")),
            &manifest,
            chunk,
        );
    }
}

#[test]
fn omitted_complete_paths_and_snapshot_bodies_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let (source, _) = source(directory.path());
    let manifest = source.record_reader(1).unwrap().manifest().clone();
    let documents = support::frames(&source, 1);
    let mut omitted = documents.clone();
    let edge = omitted
        .iter()
        .position(|document| document.kind().unwrap() == "activity.edge")
        .unwrap();
    let next = omitted[edge + 1..]
        .iter()
        .position(|document| document.kind().unwrap() == "activity.edge")
        .unwrap()
        + edge
        + 1;
    let first_path = omitted[edge + 1..next]
        .iter()
        .position(|document| document.kind().unwrap() == "activity.dependency-path")
        .unwrap()
        + edge
        + 1;
    omitted.drain(first_path..next);
    let mut wire: serde_json::Value =
        serde_json::from_slice(&omitted[edge].canonical_bytes().unwrap()).unwrap();
    wire["dependency_path_count"] = "0".into();
    omitted[edge] =
        Document::parse(&serde_json::to_vec(&wire).unwrap(), Limits::default()).unwrap();
    let (changed, chunk) = support::rehashed(&manifest, &omitted);
    rejected(
        &source,
        &directory.path().join("omitted-closure.pproj"),
        &changed,
        chunk,
    );
    for (index, kind) in ["fingerprint.snapshot", "activity.dependency-segment"]
        .into_iter()
        .enumerate()
    {
        let mut missing = documents.clone();
        let start = missing
            .iter()
            .position(|document| document.kind().unwrap() == "activity.header")
            .unwrap();
        let removed = missing[start..]
            .iter()
            .position(|document| document.kind().unwrap() == kind)
            .unwrap()
            + start;
        missing.remove(removed);
        let (changed, chunk) = support::rehashed(&manifest, &missing);
        rejected(
            &source,
            &directory.path().join(format!("missing-{index}.pproj")),
            &changed,
            chunk,
        );
    }
}
