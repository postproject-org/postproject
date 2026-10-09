//! Distinct files retain complete media and authored mixed-operation history.

use postproject_core::{
    Asset, AssetId, ContentStructure, FileFacts, FrameRange, ImageSequenceDescriptor, Locator,
    LocatorAvailability, LocatorId, MediaRoot, MediaRootId, MetadataProperty, MetadataValue,
    ObjectRef, OriginalMediaImport, PropertyId, RationalRate, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationImport, RepresentationKind,
    Resource, ResourceFingerprint, ResourceId, ResourceMember, ResourceRole, SequenceNaming,
    Timestamp, VocabularyId,
};
use postproject_protocol::{
    Document, Extensions, FrameDecoder, Limits, MediaChange, RecordChunk, RecordFeature,
    RecordManifest, ResourceCreationStart,
};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

fn fixture(content: ContentStructure) -> OriginalMediaImport {
    let asset = Asset::new(
        AssetId::new(),
        Timestamp::from_unix_micros(-7),
        Some("Original \0 名".into()),
        Some("unknown:origin".into()),
    );
    let resources: Vec<_> = content
        .resource_ids()
        .into_iter()
        .rev()
        .map(|id| {
            Resource::new(
                id,
                vec![
                    ResourceFingerprint::new("Unknown_Resource_Hash", u16::MAX, vec![255, 0])
                        .unwrap(),
                ],
                None,
            )
        })
        .collect();
    let locators = resources
        .iter()
        .map(|resource| {
            let locator = Locator::new(
                LocatorId::new(),
                resource.id(),
                format!("file:///nonexistent/media/{}", resource.id()),
                Some(Timestamp::from_unix_micros(-99)),
                LocatorAvailability::Offline,
            )
            .unwrap();
            if content.image_sequence_descriptor().is_some() {
                locator.with_sequence_naming(SequenceNaming::new("shot.", ".exr", 4).unwrap())
            } else {
                locator
            }
        })
        .collect();
    OriginalMediaImport::new(
        asset.clone(),
        Representation::new(
            RepresentationId::new(),
            asset.id(),
            RepresentationKind::Original,
            content,
            vec![
                RepresentationFingerprint::new("Unknown_Representation_Hash", 7, vec![0, 255])
                    .unwrap(),
            ],
        ),
        resources,
        locators,
    )
    .unwrap()
}

#[test]
fn all_native_content_shapes_and_original_observations_converge_without_media_io() {
    let first = ResourceId::new();
    let second = ResourceId::new();
    let member = |id, required| {
        ResourceMember::new(id, ResourceRole::new("unknown:member").unwrap(), required)
    };
    let contents = [
        ContentStructure::single_resource(first),
        ContentStructure::image_sequence(
            ImageSequenceDescriptor::new(
                first,
                FrameRange::new(-10, 10, 2).unwrap(),
                RationalRate::new(24000, 1001).unwrap(),
                vec![-2, 4],
            )
            .unwrap(),
        ),
        ContentStructure::ordered_parts(vec![member(second, true), member(first, true)]).unwrap(),
        ContentStructure::package(vec![member(first, true), member(second, false)]).unwrap(),
    ];
    for content in contents {
        assert_creation_replay(&fixture(content));
    }
}

fn assert_creation_replay(import: &OriginalMediaImport) {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let mirror_path = directory.path().join("mirror.pproj");
    let mut source = SqliteProduction::create(&source_path, None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &mirror_path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let root = MediaRoot::new(
        MediaRootId::new(),
        "rushes",
        None,
        Some("file:///retained/legacy".into()),
        i32::MAX,
        true,
    )
    .unwrap();
    let property = MetadataProperty::new(
        VocabularyId::new("unknown:名").unwrap(),
        PropertyId::new("Exact").unwrap(),
    );
    let target = ObjectRef::Asset(import.asset().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(import).unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::u64(u64::MAX))
        .unwrap();
    edit.add_media_root(root.clone()).unwrap();
    let revision = edit.commit().unwrap().revision().unwrap().id();
    drop(edit);
    let mut reader = source.record_reader(1).unwrap();
    let manifest = reader.manifest().clone();
    assert_eq!(manifest.effect_count(), 3);
    assert!(manifest.event_count() > 3);
    assert_eq!(
        manifest.required_features().collect::<Vec<_>>(),
        [
            RecordFeature::Media,
            RecordFeature::Metadata,
            RecordFeature::RecordChunks
        ]
    );
    assert!(
        mirror
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
    assert_eq!(mirror.asset(import.asset().id()).unwrap(), *import.asset());
    assert_eq!(
        mirror.representation(import.representation().id()).unwrap(),
        *import.representation()
    );
    assert_eq!(
        mirror.resources(import.representation().id()).unwrap(),
        source.resources(import.representation().id()).unwrap()
    );
    for resource in import.resources() {
        assert_eq!(
            mirror.locators(resource.id()).unwrap(),
            source.locators(resource.id()).unwrap()
        );
    }
    assert_eq!(mirror.media_roots().unwrap(), [root]);
    assert_eq!(
        mirror.metadata_values(target, &property).unwrap(),
        [MetadataValue::u64(u64::MAX)]
    );
    assert_eq!(
        mirror.events_for_revision(revision).unwrap(),
        source.events_for_revision(revision).unwrap()
    );
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    drop(mirror);
    let mut mirror = SqliteProduction::open(&mirror_path).unwrap();
    let mut reader = source.record_reader(1).unwrap();
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

fn apply_sequence(source: &SqliteProduction, mirror: &mut SqliteProduction, sequence: u64) {
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
    assert_eq!(mirror.exchange_head().unwrap(), manifest.head().unwrap());
}

#[test]
fn representation_addition_and_same_root_locator_transitions_keep_history_and_guards() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let mirror_path = directory.path().join("mirror.pproj");
    let mut source = SqliteProduction::create(&source_path, None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &mirror_path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let original = fixture(ContentStructure::single_resource(ResourceId::new()));
    let root = MediaRoot::new(
        MediaRootId::new(),
        "rushes",
        Some("Exact".into()),
        None,
        -7,
        true,
    )
    .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&original).unwrap();
    edit.add_media_root(root.clone()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    apply_sequence(&source, &mut mirror, 1);

    let (_, representation, resources, locators) =
        fixture(ContentStructure::single_resource(ResourceId::new())).into_parts();
    let proxy = RepresentationImport::new(
        Representation::new(
            representation.id(),
            original.asset().id(),
            RepresentationKind::Proxy,
            representation.content_structure().clone(),
            representation.fingerprints().to_vec(),
        ),
        resources,
        locators,
    )
    .unwrap();
    let locator = Locator::new(
        LocatorId::new(),
        proxy.resources()[0].id(),
        "unknown:alternate-copy",
        None,
        LocatorAvailability::Unknown,
    )
    .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.set_media_root_enabled(root.id(), false).unwrap();
    edit.add_representation(&proxy).unwrap();
    let first_facts = FileFacts::new(7, Some(Timestamp::from_unix_micros(-123)));
    let last_facts = FileFacts::new(99, None);
    assert!(
        edit.record_resource_file_facts(proxy.resources()[0].id(), first_facts)
            .unwrap()
    );
    assert!(
        edit.record_resource_file_facts(proxy.resources()[0].id(), last_facts)
            .unwrap()
    );
    edit.add_locator(&locator).unwrap();
    edit.retire_locator(proxy.locators()[0].id()).unwrap();
    edit.set_media_root_enabled(root.id(), true).unwrap();
    edit.remove_media_root(root.id()).unwrap();
    let revision = edit.commit().unwrap().revision().unwrap().id();
    drop(edit);
    apply_sequence(&source, &mut mirror, 2);
    assert_authored_file_facts(&source, proxy.resources()[0].id(), first_facts, last_facts);
    assert_eq!(
        mirror.representation(proxy.representation().id()).unwrap(),
        *proxy.representation()
    );
    assert_eq!(
        mirror.locators(proxy.resources()[0].id()).unwrap(),
        [locator]
    );
    assert_eq!(mirror.media_roots().unwrap(), Vec::<MediaRoot>::new());
    assert_eq!(
        mirror.events_for_revision(revision).unwrap(),
        source.events_for_revision(revision).unwrap()
    );
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    assert_eq!(guard_rows(&mirror_path), guard_rows(&source_path));
    assert_eq!(
        mirror.resources(proxy.representation().id()).unwrap()[0].file_facts(),
        Some(last_facts)
    );
}

fn assert_authored_file_facts(
    source: &SqliteProduction,
    resource_id: ResourceId,
    first: FileFacts,
    last: FileFacts,
) {
    let frames = record_frames(source, 2);
    let facts: Vec<_> = frames
        .iter()
        .filter(|frame| frame.kind().unwrap() == "resource.file-facts")
        .map(|frame| MediaChange::from_document(frame).unwrap())
        .collect();
    assert_eq!(
        facts,
        [
            MediaChange::ResourceFileFacts {
                resource_id,
                facts: first
            },
            MediaChange::ResourceFileFacts {
                resource_id,
                facts: last
            }
        ]
    );
    let creation = frames
        .iter()
        .find(|frame| frame.kind().unwrap() == "resource.creation")
        .unwrap();
    assert_eq!(
        ResourceCreationStart::from_document(creation)
            .unwrap()
            .resource()
            .file_facts(),
        None
    );
}

fn record_frames(source: &SqliteProduction, sequence: u64) -> Vec<Document> {
    let mut reader = source.record_reader(sequence).unwrap();
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut frames = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            if let Some(document) = document {
                frames.push(document);
            }
        }
    }
    decoder.finish().unwrap();
    frames
}

fn guard_rows(path: &std::path::Path) -> Vec<(Vec<u8>, Vec<u8>, i64)> {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection.prepare("SELECT conflict_key, last_changed_revision_id, last_changed_revision_sequence FROM conflict_versions ORDER BY conflict_key").unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap()
        .map(Result::unwrap).collect()
}

fn rehashed_record(
    original: &RecordManifest,
    frames: &[Document],
) -> (RecordManifest, RecordChunk) {
    let mut payload = Vec::new();
    for frame in frames {
        let bytes = frame.canonical_bytes().unwrap();
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

#[test]
fn rehashed_contradictions_and_missing_required_codec_never_publish_a_media_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let import = fixture(ContentStructure::single_resource(ResourceId::new()));
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let original = source.record_reader(1).unwrap().manifest().clone();
    let frames = record_frames(&source, 1);
    let foreign_asset = AssetId::new().to_string();
    let foreign_resource = ResourceId::new().to_string();
    let asset = import.asset().id().to_string();
    let resource = import.resources()[0].id().to_string();
    let contradictions = [
        (
            "representation.creation",
            "\"role\":\"original\"",
            "\"role\":\"proxy\"",
        ),
        (
            "representation.creation",
            asset.as_str(),
            foreign_asset.as_str(),
        ),
        ("locator.fact", resource.as_str(), foreign_resource.as_str()),
        (
            "fingerprint.observation",
            "\"observed_revision_sequence\":\"1\"",
            "\"observed_revision_sequence\":\"2\"",
        ),
        ("observation", asset.as_str(), foreign_asset.as_str()),
        (
            "resource.creation",
            "\"file_facts\":null",
            "\"file_facts\":{\"size_bytes\":\"18446744073709551615\",\"modified_at_micros\":null}",
        ),
    ];
    for (index, (kind, before, after)) in contradictions.into_iter().enumerate() {
        let mut forged = frames.clone();
        let frame = forged
            .iter_mut()
            .find(|frame| frame.kind().unwrap() == kind)
            .unwrap();
        let text = String::from_utf8(frame.canonical_bytes().unwrap()).unwrap();
        assert!(text.contains(before));
        *frame = Document::parse(
            text.replacen(before, after, 1).as_bytes(),
            Limits::default(),
        )
        .unwrap();
        let (manifest, chunk) = rehashed_record(&original, &forged);
        assert_rejected_prefix(
            &source,
            &directory.path().join(format!("forgery-{index}.pproj")),
            &manifest,
            chunk,
        );
    }
    let (manifest, chunk) = rehashed_record(&original, &frames);
    let manifest = manifest
        .with_required_features([RecordFeature::Metadata, RecordFeature::RecordChunks])
        .unwrap();
    assert_rejected_prefix(
        &source,
        &directory.path().join("missing-feature.pproj"),
        &manifest,
        chunk,
    );
}

fn assert_rejected_prefix(
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
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_floor().unwrap()
    );
    let connection = rusqlite::Connection::open(path).unwrap();
    for table in [
        "assets",
        "resources",
        "representations",
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
        assert_eq!(count, 0, "visible prefix in {table}");
    }
}

#[test]
fn failed_native_record_persistence_rolls_back_the_complete_media_commit() {
    for table in ["exchange_records", "exchange_record_chunks"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.pproj");
        let mut source = SqliteProduction::create(&path, None).unwrap();
        let import = fixture(ContentStructure::single_resource(ResourceId::new()));
        let root = MediaRoot::new(MediaRootId::new(), "rushes", None, None, 0, true).unwrap();
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(&format!("CREATE TRIGGER reject_capture BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
        let mut edit = source.begin_transaction().unwrap();
        edit.import_original(&import).unwrap();
        edit.add_media_root(root.clone()).unwrap();
        assert!(edit.commit().is_err());
        assert_eq!(edit.state(), postproject_core::TransactionState::RolledBack);
        assert!(edit.commit().is_err());
        drop(edit);
        assert_eq!(
            source.exchange_head().unwrap(),
            source.exchange_floor().unwrap()
        );
        assert_eq!(source.assets().unwrap(), []);
        assert_eq!(source.media_roots().unwrap(), []);
        assert_eq!(source.changes_since(0, 10).unwrap(), []);
        assert_eq!(guard_rows(&path), []);
        connection
            .execute_batch("DROP TRIGGER reject_capture")
            .unwrap();
        let mut edit = source.begin_transaction().unwrap();
        edit.import_original(&import).unwrap();
        edit.add_media_root(root).unwrap();
        assert_eq!(edit.commit().unwrap().revision().unwrap().sequence(), 1);
        drop(edit);
        assert_eq!(source.exchange_head().unwrap().sequence(), 1);
    }
}
