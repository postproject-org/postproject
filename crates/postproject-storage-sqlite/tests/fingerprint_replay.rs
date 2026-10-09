//! Replay preserves authored observations, supersessions and dirty clears.

use postproject_core::{
    RepresentationFingerprint, RepresentationId, ResourceFingerprint, ResourceId,
};
use postproject_media::prepare_original_media;
use postproject_protocol::{
    Document, Extensions, FingerprintChangeStart, FrameDecoder, Limits, RecordChunk, RecordManifest,
};
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

#[test]
fn rehashed_fingerprint_contradictions_never_publish_any_observation_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let media = directory.path().join("clip.mov");
    std::fs::write(&media, b"original").unwrap();
    let import = prepare_original_media(&media, None, None).unwrap();
    let resource = &import.resources()[0];
    let initial = &resource.fingerprints()[0];
    let changed =
        ResourceFingerprint::new(initial.algorithm(), initial.version(), vec![99]).unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_resource_fingerprint(resource.id(), &changed)
        .unwrap();
    edit.record_representation_fingerprint(
        import.representation().id(),
        &import.representation().fingerprints()[0],
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let original = source.record_reader(2).unwrap().manifest().clone();
    let original_frames = frames(&source, 2);
    let resource_id = resource.id().to_string();
    let foreign = ResourceId::new().to_string();
    let contradictions = [
        (
            "fingerprint.change",
            0,
            "\"archived_position\":\"0\"",
            "\"archived_position\":\"1\"",
        ),
        (
            "fingerprint.change",
            0,
            "\"observed_revision_sequence\":\"1\"",
            "\"observed_revision_sequence\":null",
        ),
        (
            "fingerprint.change",
            0,
            "\"observed_revision_sequence\":\"2\"",
            "\"observed_revision_sequence\":\"1\"",
        ),
        (
            "fingerprint.change",
            0,
            "\"marker_count\":\"1\"",
            "\"marker_count\":\"2\"",
        ),
        (
            "fingerprint.recomputation",
            0,
            resource_id.as_str(),
            foreign.as_str(),
        ),
        (
            "fingerprint.recomputation",
            0,
            "\"revision_sequence\":\"2\"",
            "\"revision_sequence\":\"1\"",
        ),
        (
            "fingerprint.change",
            1,
            "\"revision_sequence\":\"2\"",
            "\"revision_sequence\":\"1\"",
        ),
        (
            "fingerprint.change",
            1,
            "\"dependency_invalidated\":false",
            "\"dependency_invalidated\":true",
        ),
    ];
    for (index, (kind, occurrence, before, after)) in contradictions.into_iter().enumerate() {
        let mut forged = original_frames.clone();
        let frame = forged
            .iter_mut()
            .filter(|frame| frame.kind().unwrap() == kind)
            .nth(occurrence)
            .unwrap();
        let encoded = String::from_utf8(frame.canonical_bytes().unwrap()).unwrap();
        assert!(encoded.contains(before));
        *frame = Document::parse(
            encoded.replacen(before, after, 1).as_bytes(),
            Limits::default(),
        )
        .unwrap();
        assert_forgery(
            &source,
            &import,
            &original,
            &forged,
            &directory.path().join(format!("mirror-{index}.pproj")),
        );
    }
    let mut truncated = original_frames;
    truncated.retain(|frame| frame.kind().unwrap() != "fingerprint.recomputation");
    assert_forgery(
        &source,
        &import,
        &original,
        &truncated,
        &directory.path().join("missing-marker.pproj"),
    );
}

fn assert_forgery(
    source: &SqliteProduction,
    import: &postproject_core::OriginalMediaImport,
    original: &RecordManifest,
    forged: &[Document],
    path: &std::path::Path,
) {
    let (manifest, chunk) = rehashed(original, forged);
    let mut mirror = SqliteProduction::create_genesis_mirror(
        path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    apply(source, &mut mirror, 1);
    let prior = mirror.exchange_head().unwrap();
    assert!(
        mirror
            .apply_record(&manifest, [Ok(chunk)], ReplayLimits::default())
            .is_err()
    );
    assert_eq!(mirror.exchange_head().unwrap(), prior);
    assert_eq!(
        mirror.resources(import.representation().id()).unwrap(),
        import.resources()
    );
    assert_eq!(mirror.changes_since(0, 10).unwrap().len(), 1);
    assert_no_fingerprint_prefix(path);
}

fn assert_no_fingerprint_prefix(path: &std::path::Path) {
    let connection = Connection::open(path).unwrap();
    for table in [
        "resource_fingerprint_history",
        "representation_fingerprint_history",
        "representation_fingerprint_recomputations",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "visible forged prefix in {table}");
    }
}

fn rehashed(original: &RecordManifest, documents: &[Document]) -> (RecordManifest, RecordChunk) {
    let mut payload = Vec::new();
    for document in documents {
        let bytes = document.canonical_bytes().unwrap();
        payload.extend_from_slice(&u64::try_from(bytes.len()).unwrap().to_be_bytes());
        payload.extend(bytes);
    }
    let chunk = RecordChunk::new(
        original.predecessor().scope(),
        original.revision().id(),
        0,
        None,
        payload,
        Extensions::default(),
    )
    .unwrap();
    let mut chain = original.chunk_chain();
    chain.push(&chunk).unwrap();
    let manifest = RecordManifest::new(
        original.predecessor(),
        original.revision().clone(),
        chain.finish().unwrap(),
        original.effect_count(),
        original.event_count(),
        original.extensions().clone(),
    )
    .unwrap()
    .with_required_features(original.required_features())
    .unwrap();
    (manifest, chunk)
}
