//! Replay preserves authored observations, supersessions and dirty clears.

use postproject_core::{
    RepresentationFingerprint, RepresentationId, ResourceFingerprint, ResourceId,
};
use postproject_media::prepare_original_media;
use postproject_protocol::{Document, FingerprintChangeStart, FrameDecoder, Limits};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};
use rusqlite::Connection;

#[test]
fn same_revision_supersessions_and_unchanged_dirty_clear_converge() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let mirror_path = directory.path().join("mirror.pproj");
    let media = directory.path().join("clip.mov");
    std::fs::write(&media, b"original").unwrap();
    let import = prepare_original_media(&media, None, None).unwrap();
    let resource = &import.resources()[0];
    let representation = import.representation();
    let initial_resource = &resource.fingerprints()[0];
    let initial_representation = &representation.fingerprints()[0];
    let first = ResourceFingerprint::new(
        initial_resource.algorithm(),
        initial_resource.version(),
        vec![99],
    )
    .unwrap();
    let last = ResourceFingerprint::new(
        initial_resource.algorithm(),
        initial_resource.version(),
        vec![88],
    )
    .unwrap();
    let intermediate = RepresentationFingerprint::new(
        initial_representation.algorithm(),
        initial_representation.version(),
        vec![77],
    )
    .unwrap();
    let final_value = RepresentationFingerprint::new(
        initial_representation.algorithm(),
        initial_representation.version(),
        vec![66],
    )
    .unwrap();
    let mut source = SqliteProduction::create(&source_path, None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &mirror_path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    apply(&source, &mut mirror, 1);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_resource_fingerprint(resource.id(), &first)
            .unwrap()
    );
    assert!(
        edit.record_representation_fingerprint(representation.id(), initial_representation)
            .unwrap()
    );
    assert!(
        edit.record_resource_fingerprint(resource.id(), &last)
            .unwrap()
    );
    assert!(
        edit.record_representation_fingerprint(representation.id(), &intermediate)
            .unwrap()
    );
    assert!(
        edit.record_representation_fingerprint(representation.id(), &final_value)
            .unwrap()
    );
    let revision = edit.commit().unwrap().revision().unwrap().id();
    drop(edit);
    apply(&source, &mut mirror, 2);
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert_eq!(
        mirror.events_for_revision(revision).unwrap(),
        source.events_for_revision(revision).unwrap()
    );
    assert_eq!(
        mirror.resources(representation.id()).unwrap(),
        source.resources(representation.id()).unwrap()
    );
    assert_eq!(
        mirror.representation(representation.id()).unwrap(),
        source.representation(representation.id()).unwrap()
    );
    assert_evidence_rows_equal(&source_path, &mirror_path);
    assert_authored_boundaries(&source);
    assert_noop(
        &mut source,
        resource.id(),
        &last,
        representation.id(),
        &final_value,
    );
    drop(mirror);
    let mut mirror = SqliteProduction::open(&mirror_path).unwrap();
    assert_duplicate(&source, &mut mirror);
    assert_evidence_rows_equal(&source_path, &mirror_path);
}

fn assert_noop(
    source: &mut SqliteProduction,
    resource: ResourceId,
    last: &ResourceFingerprint,
    representation: RepresentationId,
    final_value: &RepresentationFingerprint,
) {
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(!edit.record_resource_fingerprint(resource, last).unwrap());
    assert!(
        !edit
            .record_representation_fingerprint(representation, final_value)
            .unwrap()
    );
    assert!(edit.commit().unwrap().revision().is_none());
    drop(edit);
}

fn assert_authored_boundaries(source: &SqliteProduction) {
    let changes: Vec<_> = frames(source, 2)
        .iter()
        .filter(|document| document.kind().unwrap() == "fingerprint.change")
        .map(|document| FingerprintChangeStart::from_document(document).unwrap())
        .collect();
    assert_eq!(changes.len(), 5);
    assert_eq!(changes[0].archived_position(), Some(0));
    assert_eq!(changes[1].current().observed_revision_sequence(), Some(1));
    assert_eq!(changes[1].archived_position(), None);
    assert_eq!(changes[1].cleared_marker().unwrap().revision_sequence(), 2);
    assert_eq!(changes[2].archived_position(), Some(1));
    assert_eq!(changes[4].archived_position(), Some(1));
}

fn assert_duplicate(source: &SqliteProduction, mirror: &mut SqliteProduction) {
    let mut reader = source.record_reader(2).unwrap();
    let manifest = reader.manifest().clone();
    assert!(
        !mirror
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
}

fn apply(source: &SqliteProduction, mirror: &mut SqliteProduction, sequence: u64) {
    let mut reader = source.record_reader(sequence).unwrap();
    let manifest = reader.manifest().clone();
    assert!(
        mirror
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
}

fn frames(source: &SqliteProduction, sequence: u64) -> Vec<Document> {
    let mut reader = source.record_reader(sequence).unwrap();
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut documents = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (read, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += read;
            if let Some(document) = document {
                documents.push(document);
            }
        }
    }
    decoder.finish().unwrap();
    documents
}

fn assert_evidence_rows_equal(source: &std::path::Path, mirror: &std::path::Path) {
    let source = Connection::open(source).unwrap();
    let mirror = Connection::open(mirror).unwrap();
    for (table, columns, order) in [
        (
            "resource_fingerprints",
            "resource_id,algorithm,algorithm_version,value,observed_revision_sequence",
            "resource_id,algorithm,algorithm_version",
        ),
        (
            "representation_fingerprints",
            "representation_id,algorithm,algorithm_version,value,observed_revision_sequence",
            "representation_id,algorithm,algorithm_version",
        ),
        (
            "resource_fingerprint_history",
            "resource_id,algorithm,algorithm_version,value,observed_revision_sequence,superseded_revision_sequence",
            "id",
        ),
        (
            "representation_fingerprint_history",
            "representation_id,algorithm,algorithm_version,value,observed_revision_sequence,superseded_revision_sequence",
            "id",
        ),
        (
            "representation_fingerprint_recomputations",
            "representation_id,changed_resource_id,marked_revision_sequence",
            "representation_id",
        ),
        (
            "conflict_versions",
            "conflict_key,last_changed_revision_id,last_changed_revision_sequence",
            "conflict_key",
        ),
    ] {
        let query = format!("SELECT {columns} FROM {table} ORDER BY {order}");
        let rows = |connection: &Connection| {
            let mut statement = connection.prepare(&query).unwrap();
            let count = statement.column_count();
            statement
                .query_map([], |row| {
                    (0..count)
                        .map(|column| row.get::<_, rusqlite::types::Value>(column))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(
            rows(&source),
            rows(&mirror),
            "different original evidence in {table}"
        );
    }
}
