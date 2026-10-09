//! All content shapes retain mutable and historical facts through both routes.

use postproject_core::{
    Asset, AssetId, ContentStructure, ExternalIdentifier, FileFacts, FrameRange, IdentifierScheme,
    ImageSequenceDescriptor, Locator, LocatorAvailability, LocatorId, MetadataProperty,
    MetadataValue, ObjectRef, OriginalMediaImport, PropertyId, RationalRate, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationKind, Resource, ResourceFingerprint,
    ResourceId, ResourceMember, ResourceRole, SequenceNaming, Timestamp, VocabularyId,
};
use postproject_storage_sqlite::{CheckpointLimits, ReplayLimits, SqliteProduction};

#[test]
fn all_shapes_converge_from_early_checkpoint_suffix_and_later_checkpoint() {
    let first = ResourceId::new();
    let second = ResourceId::new();
    let member = |id, required| {
        ResourceMember::new(id, ResourceRole::new("unknown:role").unwrap(), required)
    };
    for content in [
        ContentStructure::single_resource(first),
        ContentStructure::image_sequence(
            ImageSequenceDescriptor::new(
                first,
                FrameRange::new(-10, 10, 2).unwrap(),
                RationalRate::new(24_000, 1_001).unwrap(),
                vec![-2, 4],
            )
            .unwrap(),
        ),
        ContentStructure::ordered_parts(vec![member(second, true), member(first, true)]).unwrap(),
        ContentStructure::package(vec![member(first, true), member(second, false)]).unwrap(),
    ] {
        compare_routes(&fixture(content));
    }
}

fn compare_routes(import: &OriginalMediaImport) {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let target = ObjectRef::Asset(import.asset().id());
    let property = MetadataProperty::new(
        VocabularyId::new("unknown:CASE").unwrap(),
        PropertyId::new("Exact").unwrap(),
    );
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("unknown:CASE").unwrap(),
        "Exact 名",
        Some("Qual".into()),
    )
    .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(import).unwrap();
    edit.add_external_identifier(target, &identifier).unwrap();
    for _ in 0..2 {
        edit.add_metadata_value(target, &property, &MetadataValue::u64(u64::MAX))
            .unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    let mut early = checkpoint(&source, &directory.path().join("early.pproj"));
    equivalent(&source, &early, import, &property);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    let resource = import.resources()[0].id();
    edit.record_resource_fingerprint(
        resource,
        &ResourceFingerprint::new("content", 1, vec![2]).unwrap(),
    )
    .unwrap();
    edit.record_representation_fingerprint(
        import.representation().id(),
        &RepresentationFingerprint::new("aggregate", 1, vec![12]).unwrap(),
    )
    .unwrap();
    edit.record_resource_file_facts(
        resource,
        FileFacts::new(456, Some(Timestamp::from_unix_micros(-99))),
    )
    .unwrap();
    edit.remove_external_identifier(target, &identifier)
        .unwrap();
    edit.add_external_identifier(target, &identifier).unwrap();
    let original = &import.locators()[0];
    edit.retire_locator(original.id()).unwrap();
    let replacement = locator(
        original.id(),
        resource,
        "file:///replacement.mov",
        original.sequence_naming().is_some(),
    );
    edit.add_locator(&replacement).unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(i64::MIN))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut reader = source.record_reader(2).unwrap();
    let manifest = reader.manifest().clone();
    assert!(
        early
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
    let late_path = directory.path().join("late.pproj");
    let late = checkpoint(&source, &late_path);
    for mirror in [&early, &late] {
        equivalent(&source, mirror, import, &property);
    }
    drop(late);
    equivalent(
        &source,
        &SqliteProduction::open(late_path).unwrap(),
        import,
        &property,
    );
    assert!(early.begin_transaction().is_err());
}

fn checkpoint(source: &SqliteProduction, path: &std::path::Path) -> SqliteProduction {
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let second = source.export_checkpoint(|_| Ok(())).unwrap();
    assert_eq!(manifest.head(), second.head());
    assert_ne!(manifest.id(), second.id());
    SqliteProduction::import_checkpoint(
        path,
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap()
}

fn equivalent(
    source: &SqliteProduction,
    mirror: &SqliteProduction,
    import: &OriginalMediaImport,
    property: &MetadataProperty,
) {
    let asset = import.asset().id();
    let representation = import.representation().id();
    assert_eq!(source.production(), mirror.production());
    assert_eq!(source.asset(asset).unwrap(), mirror.asset(asset).unwrap());
    assert_eq!(
        source.representation(representation).unwrap(),
        mirror.representation(representation).unwrap()
    );
    assert_eq!(
        source.resources(representation).unwrap(),
        mirror.resources(representation).unwrap()
    );
    for resource in import.resources() {
        assert_eq!(
            source.locators(resource.id()).unwrap(),
            mirror.locators(resource.id()).unwrap()
        );
    }
    let target = ObjectRef::Asset(asset);
    assert_eq!(
        source.external_identifiers(target).unwrap(),
        mirror.external_identifiers(target).unwrap()
    );
    assert_eq!(
        source.metadata_values(target, property).unwrap(),
        mirror.metadata_values(target, property).unwrap()
    );
    let revisions = source.changes_since(0, 10).unwrap();
    assert_eq!(revisions, mirror.changes_since(0, 10).unwrap());
    for revision in revisions {
        assert_eq!(
            source.events_for_revision(revision.id()).unwrap(),
            mirror.events_for_revision(revision.id()).unwrap()
        );
    }
    assert_eq!(
        source.exchange_head().unwrap(),
        mirror.exchange_head().unwrap()
    );
}

fn fixture(content: ContentStructure) -> OriginalMediaImport {
    let asset = Asset::new(
        AssetId::new(),
        Timestamp::from_unix_micros(-1),
        Some("Exact 名".into()),
        None,
    );
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        content,
        vec![RepresentationFingerprint::new("aggregate", 1, vec![11]).unwrap()],
    );
    let resources: Vec<_> = representation
        .content_structure()
        .resource_ids()
        .into_iter()
        .map(|id| {
            Resource::new(
                id,
                vec![ResourceFingerprint::new("content", 1, vec![1]).unwrap()],
                Some(FileFacts::new(123, None)),
            )
        })
        .collect();
    let sequence = representation
        .content_structure()
        .image_sequence_descriptor()
        .is_some();
    let locators = resources
        .iter()
        .map(|resource| {
            locator(
                LocatorId::new(),
                resource.id(),
                "file:///missing.mov",
                sequence,
            )
        })
        .collect();
    OriginalMediaImport::new(asset, representation, resources, locators).unwrap()
}

fn locator(id: LocatorId, resource: ResourceId, uri: &str, sequence: bool) -> Locator {
    let locator = Locator::new(id, resource, uri, None, LocatorAvailability::Offline).unwrap();
    if sequence {
        locator.with_sequence_naming(SequenceNaming::new("shot.", ".exr", 4).unwrap())
    } else {
        locator
    }
}
