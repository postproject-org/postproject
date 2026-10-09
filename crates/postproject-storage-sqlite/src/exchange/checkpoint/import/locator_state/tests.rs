use postproject_core::{
    Asset, AssetId, ContentStructure, FrameRange, ImageSequenceDescriptor, Locator,
    LocatorAvailability, LocatorId, MediaRoot, MediaRootId, OriginalMediaImport, RationalRate,
    Representation, RepresentationId, RepresentationKind, Resource, ResourceId, SequenceNaming,
    Timestamp,
};

use crate::SqliteProduction;

#[test]
fn retirement_and_explicit_identity_reuse_check_the_latest_exact_locator() {
    let (_directory, mut source, import) = fixture(false);
    let original = &import.locators()[0];
    super::create(&source.connection).unwrap();
    super::added(&source.connection, original).unwrap();
    assert!(super::added(&source.connection, original).is_err());
    assert!(super::retired(&source.connection, original.id(), ResourceId::new(), true).is_err());
    super::retired(
        &source.connection,
        original.id(),
        original.resource_id(),
        true,
    )
    .unwrap();
    assert!(
        super::retired(
            &source.connection,
            original.id(),
            original.resource_id(),
            true
        )
        .is_err()
    );
    let replacement = Locator::new(
        original.id(),
        original.resource_id(),
        "file:///replacement.mov",
        Some(Timestamp::from_unix_micros(-99)),
        LocatorAvailability::Offline,
    )
    .unwrap();
    super::added(&source.connection, &replacement).unwrap();
    assert!(super::finish(&source.connection, true).is_err());
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.retire_locator(original.id()).unwrap();
    edit.add_locator(&replacement).unwrap();
    edit.commit().unwrap();
    drop(edit);
    super::finish(&source.connection, true).unwrap();
}

#[test]
fn sequence_naming_is_exact_and_matches_the_immutable_content_kind() {
    let (_directory, source, import) = fixture(true);
    super::create(&source.connection).unwrap();
    let original = &import.locators()[0];
    let unnamed = Locator::new(
        original.id(),
        original.resource_id(),
        original.uri(),
        None,
        original.availability(),
    )
    .unwrap();
    assert!(super::added(&source.connection, &unnamed).is_err());
    super::added(&source.connection, original).unwrap();
    let duplicate = Locator::new(
        LocatorId::new(),
        original.resource_id(),
        original.uri(),
        None,
        original.availability(),
    )
    .unwrap()
    .with_sequence_naming(original.sequence_naming().unwrap().clone());
    assert!(super::added(&source.connection, &duplicate).is_err());
    source
        .connection
        .execute("UPDATE locator_sequence_namings SET padding = 5", [])
        .unwrap();
    assert!(super::finish(&source.connection, true).is_err());
    source
        .connection
        .execute("UPDATE locator_sequence_namings SET padding = 4", [])
        .unwrap();
    super::finish(&source.connection, true).unwrap();
}

#[test]
fn migration_retirements_do_not_resurrect_unknown_earlier_locators() {
    let (_directory, mut source, import) = fixture(false);
    super::create(&source.connection).unwrap();
    let original = &import.locators()[0];
    assert!(
        super::retired(
            &source.connection,
            original.id(),
            original.resource_id(),
            true
        )
        .is_err()
    );
    super::retired(
        &source.connection,
        original.id(),
        original.resource_id(),
        false,
    )
    .unwrap();
    assert!(super::finish(&source.connection, false).is_err());
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.retire_locator(original.id()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    super::finish(&source.connection, false).unwrap();
}

#[test]
fn known_root_removal_clears_expected_associations_without_reattaching_them() {
    let (_directory, mut source, import) = fixture(false);
    super::create(&source.connection).unwrap();
    let rooted = import.locators()[0]
        .clone()
        .with_media_root("rushes")
        .unwrap();
    let root = MediaRoot::new(MediaRootId::new(), "rushes", None, None, 0, true).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.add_media_root(root.clone()).unwrap();
    edit.retire_locator(rooted.id()).unwrap();
    edit.add_locator(&rooted).unwrap();
    edit.commit().unwrap();
    drop(edit);
    super::added(&source.connection, &rooted).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.remove_media_root(root.id()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert!(super::finish(&source.connection, true).is_err());
    super::clear_known_root(&source.connection, "rushes").unwrap();
    super::clear_known_root(&source.connection, "other").unwrap();
    super::finish(&source.connection, true).unwrap();
}

fn fixture(sequence: bool) -> (tempfile::TempDir, SqliteProduction, OriginalMediaImport) {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(-1), None, None);
    let resource = Resource::new(ResourceId::new(), Vec::new(), None);
    let structure = if sequence {
        ContentStructure::image_sequence(
            ImageSequenceDescriptor::new(
                resource.id(),
                FrameRange::new(-10, 10, 2).unwrap(),
                RationalRate::new(24, 1).unwrap(),
                vec![-2, 4],
            )
            .unwrap(),
        )
    } else {
        ContentStructure::single_resource(resource.id())
    };
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        structure,
        Vec::new(),
    );
    let mut locator = Locator::new(
        LocatorId::new(),
        resource.id(),
        "file:///does-not-exist",
        None,
        LocatorAvailability::Offline,
    )
    .unwrap();
    if sequence {
        locator = locator.with_sequence_naming(SequenceNaming::new("shot.", ".exr", 4).unwrap());
    }
    let import =
        OriginalMediaImport::new(asset, representation, vec![resource], vec![locator]).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    (directory, source, import)
}
