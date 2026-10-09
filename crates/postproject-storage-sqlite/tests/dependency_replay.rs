//! Complete authored dependency replacements converge in distinct passive files.

use postproject_core::{
    Dependency, DependencyKind, DependencySetStatus, DependencyTarget, OriginalMediaImport,
    RepresentationFingerprint,
};
use postproject_media::prepare_original_media;
use postproject_protocol::{
    Document, Extensions, FrameDecoder, Limits, RecordChunk, RecordFeature, RecordManifest,
};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

fn dependencies(import: &OriginalMediaImport) -> Vec<Dependency> {
    let edge = |target, resolved, required, authored| {
        Dependency::new(
            Some(import.resources()[0].id()),
            DependencyKind::new("unknown:ExAct").unwrap(),
            target,
            resolved,
            required,
            authored,
        )
        .unwrap()
    };
    vec![
        edge(
            DependencyTarget::Asset(import.asset().id()),
            Some(import.representation().id()),
            true,
            "  名/EXACT%2f  ",
        ),
        edge(
            DependencyTarget::Asset(import.asset().id()),
            None,
            false,
            "",
        ),
        edge(
            DependencyTarget::Representation(import.representation().id()),
            None,
            true,
            "self/pinned",
        ),
    ]
}

fn source(directory: &std::path::Path) -> (SqliteProduction, OriginalMediaImport) {
    let media = directory.join("clip.dat");
    std::fs::write(&media, b"dependency replay").unwrap();
    let import = prepare_original_media(&media, None, None).unwrap();
    let mut source = SqliteProduction::create(directory.join("source.pproj"), None).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.import_original(&import).unwrap();
    let representation = import.representation().id();
    assert!(edit.record_dependency_set(representation, &[]).unwrap());
    let repeated = vec![dependencies(&import)[0].clone(); 2];
    assert!(
        edit.record_dependency_set(representation, &repeated)
            .unwrap()
    );
    assert!(
        !edit
            .record_dependency_set(representation, &repeated)
            .unwrap()
    );
    assert!(edit.record_dependency_set(representation, &[]).unwrap());
    assert!(
        edit.record_dependency_set(representation, &dependencies(&import))
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    std::fs::remove_file(media).unwrap();
    (source, import)
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

#[test]
fn empty_repeated_floating_pinned_and_invalidated_observations_converge() {
    let directory = tempfile::tempdir().unwrap();
    let (mut source, import) = source(directory.path());
    let path = directory.path().join("mirror.pproj");
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let manifest = source.record_reader(1).unwrap().manifest().clone();
    assert_eq!(manifest.effect_count(), 5);
    assert_eq!(
        manifest.required_features().collect::<Vec<_>>(),
        [
            RecordFeature::Dependencies,
            RecordFeature::Media,
            RecordFeature::RecordChunks
        ]
    );
    apply(&source, &mut mirror, 1);
    let representation = import.representation().id();
    assert_eq!(
        mirror.dependency_set(representation).unwrap(),
        source.dependency_set(representation).unwrap()
    );
    let fingerprint = &import.representation().fingerprints()[0];
    let changed =
        RepresentationFingerprint::new(fingerprint.algorithm(), fingerprint.version(), vec![77])
            .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_representation_fingerprint(representation, &changed)
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    apply(&source, &mut mirror, 2);
    assert_eq!(
        mirror.dependency_set(representation).unwrap(),
        source.dependency_set(representation).unwrap()
    );
    assert_eq!(
        mirror
            .dependency_set(representation)
            .unwrap()
            .unwrap()
            .status(),
        DependencySetStatus::NeedsExtraction
    );
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_dependency_set(representation, &dependencies(&import))
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    apply(&source, &mut mirror, 3);
    let expected = source.dependency_set(representation).unwrap();
    assert_eq!(mirror.dependency_set(representation).unwrap(), expected);
    assert_eq!(expected.unwrap().recorded_at_revision(), 3);
    for sequence in 1..=3 {
        let manifest = source.record_reader(sequence).unwrap().manifest().clone();
        assert_eq!(
            mirror
                .events_for_revision(manifest.revision().id())
                .unwrap(),
            source
                .events_for_revision(manifest.revision().id())
                .unwrap()
        );
    }
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    drop(mirror);
    let mut mirror = SqliteProduction::open(&path).unwrap();
    let mut reader = source.record_reader(3).unwrap();
    assert!(
        !mirror
            .apply_record(
                &reader.manifest().clone(),
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
}

fn frames(source: &SqliteProduction) -> Vec<Document> {
    let mut reader = source.record_reader(1).unwrap();
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

fn assert_rejected(
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
        "dependency_sets",
        "dependencies",
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
        assert_eq!(count, 0, "published prefix in {table}");
    }
}

#[test]
fn rehashed_dependency_contradictions_publish_no_creation_or_history_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let (source, _) = source(directory.path());
    let original = source.record_reader(1).unwrap().manifest().clone();
    let documents = frames(&source);
    let foreign = postproject_core::RepresentationId::new().to_string();
    let contradictions: [(&str, usize, &[&str], &str); 9] = [
        ("dependency.set", 0, &["recorded_revision_sequence"], "2"),
        ("dependency.set", 0, &["status"], "needs-extraction"),
        ("dependency.set", 0, &["source_representation_id"], &foreign),
        ("dependency.set", 1, &["occurrence_count"], "0"),
        ("dependency.occurrence", 0, &["position"], "1"),
        (
            "dependency.occurrence",
            0,
            &["source_representation_id"],
            &foreign,
        ),
        (
            "dependency.occurrence",
            0,
            &["dependency", "source_resource_id"],
            &foreign,
        ),
        (
            "dependency.occurrence",
            0,
            &["dependency", "resolved_representation_id"],
            &foreign,
        ),
        ("dependency.set", 3, &["occurrence_count"], "4"),
    ];
    for (index, (kind, position, path, value)) in contradictions.into_iter().enumerate() {
        let mut altered = documents.clone();
        let document = altered
            .iter_mut()
            .filter(|document| document.kind().unwrap() == kind)
            .nth(position)
            .unwrap();
        let mut text = String::from_utf8(document.canonical_bytes().unwrap()).unwrap();
        // These fixture fields are unique, unescaped UUID/status/decimal strings.
        let marker = format!("\"{}\":\"", path.last().unwrap());
        let start = text.find(&marker).unwrap() + marker.len();
        let end = start + text[start..].find('"').unwrap();
        text.replace_range(start..end, value);
        *document = Document::parse(text.as_bytes(), Limits::default()).unwrap();
        let (manifest, chunk) = rehashed(&original, &altered);
        assert_rejected(
            &source,
            &directory.path().join(format!("forged-{index}.pproj")),
            &manifest,
            chunk,
        );
    }
    let mut missing = documents;
    let index = missing
        .iter()
        .position(|document| document.kind().unwrap() == "dependency.occurrence")
        .unwrap();
    missing.remove(index);
    let (manifest, chunk) = rehashed(&original, &missing);
    assert_rejected(
        &source,
        &directory.path().join("missing.pproj"),
        &manifest,
        chunk,
    );
}

#[test]
fn failed_dependency_staging_keeps_prior_set_guards_and_authored_queue() {
    let directory = tempfile::tempdir().unwrap();
    let (mut source, import) = source(directory.path());
    let source_path = directory.path().join("source.pproj");
    let connection = rusqlite::Connection::open(&source_path).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_dependency BEFORE INSERT ON dependencies WHEN NEW.position = 1 BEGIN SELECT RAISE(ABORT, 'injected dependency write failure'); END;").unwrap();
    let representation = import.representation().id();
    let original = source.dependency_set(representation).unwrap();
    let replacement = vec![dependencies(&import)[0].clone(); 2];
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert_eq!(
        edit.record_dependency_set(representation, &replacement)
            .unwrap_err()
            .kind(),
        postproject_core::ErrorKind::AlreadyExists
    );
    edit.add_media_root(
        postproject_core::MediaRoot::new(
            postproject_core::MediaRootId::new(),
            "independent",
            None,
            None,
            0,
            true,
        )
        .unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert_eq!(source.dependency_set(representation).unwrap(), original);
    let manifest = source.record_reader(2).unwrap().manifest().clone();
    assert_eq!(manifest.effect_count(), 1);
    assert_eq!(manifest.event_count(), 1);
    let mut key = vec![3];
    key.extend_from_slice(representation.as_bytes());
    let dependency_sequence: i64 = connection
        .query_row(
            "SELECT last_changed_revision_sequence FROM conflict_versions WHERE conflict_key = ?1",
            [key],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(dependency_sequence, 1);
    connection
        .execute_batch("DROP TRIGGER fail_dependency")
        .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_dependency_set(representation, &replacement)
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("failure-mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    for sequence in 1..=3 {
        apply(&source, &mut mirror, sequence);
    }
    assert_eq!(
        mirror.dependency_set(representation).unwrap(),
        source.dependency_set(representation).unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
}
