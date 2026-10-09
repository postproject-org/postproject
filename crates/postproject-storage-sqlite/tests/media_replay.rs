//! Distinct files retain complete media and authored mixed-operation history.

use postproject_core::{
    Asset, AssetId, ContentStructure, FrameRange, ImageSequenceDescriptor, Locator,
    LocatorAvailability, LocatorId, MediaRoot, MediaRootId, MetadataProperty, MetadataValue,
    ObjectRef, OriginalMediaImport, PropertyId, RationalRate, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationKind, Resource, ResourceFingerprint,
    ResourceId, ResourceMember, ResourceRole, SequenceNaming, Timestamp, VocabularyId,
};
use postproject_protocol::RecordFeature;
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
