use postproject_core::{
    Asset, AssetId, ContentStructure, FileFacts, Locator, LocatorAvailability, LocatorId,
    ObjectRef, OriginalMediaImport, Representation, RepresentationId, RepresentationKind, Resource,
    ResourceId, Timestamp,
};
use postproject_protocol::{RepresentationHeader, ResourceHeader};

use crate::SqliteProduction;

#[test]
fn final_presence_does_not_allow_future_creations_to_be_treated_as_baseline() {
    let directory = tempfile::tempdir().unwrap();
    let mut source =
        SqliteProduction::create(directory.path().join("presence.pproj"), None).unwrap();
    let baseline = fixture();
    let future = fixture();
    for import in [&baseline, &future] {
        let mut edit = source.begin_transaction().unwrap();
        edit.import_original(import).unwrap();
        edit.commit().unwrap();
    }
    let connection = &source.connection;
    super::create(connection).unwrap();
    let targets = |import: &OriginalMediaImport| {
        [
            ObjectRef::Asset(import.asset().id()),
            ObjectRef::Representation(import.representation().id()),
            ObjectRef::Resource(import.resources()[0].id()),
        ]
    };
    for target in targets(&baseline) {
        assert!(super::require(connection, target, 0).is_err());
        super::require(connection, target, 1).unwrap();
    }
    for target in targets(&future) {
        assert!(super::require(connection, target, 1).is_err());
    }
    audit_creation(connection, &future);
    for target in targets(&future) {
        super::require(connection, target, 1).unwrap();
    }
    assert!(super::require(connection, ObjectRef::Resource(ResourceId::new()), 1).is_err());
    super::finish(connection, false).unwrap();
}

#[test]
fn authored_creation_checks_immutable_facts_and_only_the_final_measurements() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let import = fixture();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    super::create(&source.connection).unwrap();
    audit_creation(&source.connection, &import);
    let changed = FileFacts::new(456, Some(Timestamp::from_unix_micros(-99)));
    super::changed_facts(
        &source.connection,
        import.resources()[0].id(),
        changed,
        true,
    )
    .unwrap();
    // Current rows cannot retain the old creation measurements after a change.
    assert!(super::finish(&source.connection, true).is_err());
    source
        .connection
        .execute(
            "UPDATE resources SET file_size_bytes = 456, modified_at_micros = -99",
            [],
        )
        .unwrap();
    assert!(
        super::changed_facts(
            &source.connection,
            import.resources()[0].id(),
            changed,
            true
        )
        .is_err()
    );
    super::finish(&source.connection, true).unwrap();
    let remaining: i64 = source.connection.query_row("SELECT count(*) FROM sqlite_schema WHERE name IN ('checkpoint_resource_facts', 'checkpoint_media_created')", [], |row| row.get(0)).unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn unknown_migration_creations_are_baseline_facts_without_fabricated_history() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let import = fixture();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    super::create(&source.connection).unwrap();
    assert!(super::finish(&source.connection, true).is_err());
    assert!(
        super::changed_facts(
            &source.connection,
            import.resources()[0].id(),
            FileFacts::new(123, None),
            true
        )
        .is_err()
    );
    super::changed_facts(
        &source.connection,
        import.resources()[0].id(),
        FileFacts::new(123, None),
        false,
    )
    .unwrap();
    super::finish(&source.connection, false).unwrap();
}

#[test]
fn duplicated_creations_foreign_ownership_and_changed_headers_reject() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let import = fixture();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    super::create(&source.connection).unwrap();
    audit_creation(&source.connection, &import);
    assert!(super::asset(&source.connection, import.asset()).is_err());
    assert!(
        super::resource(
            &source.connection,
            ResourceHeader::from_resource(&import.resources()[0])
        )
        .is_err()
    );
    assert!(
        super::changed_facts(
            &source.connection,
            ResourceId::new(),
            FileFacts::new(123, None),
            false
        )
        .is_err()
    );
    assert!(
        super::structure(
            &source.connection,
            import.representation().id(),
            &ContentStructure::single_resource(ResourceId::new())
        )
        .is_err()
    );
    super::finish(&source.connection, true).unwrap();
    super::create(&source.connection).unwrap();
    let altered = Asset::new(
        import.asset().id(),
        import.asset().created_at(),
        Some("Altered".into()),
        None,
    );
    assert!(super::asset(&source.connection, &altered).is_err());
    let altered_owner = RepresentationHeader::new(
        import.representation().id(),
        AssetId::new(),
        RepresentationKind::Original,
    )
    .unwrap();
    assert!(super::representation(&source.connection, altered_owner).is_err());
}

fn audit_creation(connection: &rusqlite::Connection, import: &OriginalMediaImport) {
    super::asset(connection, import.asset()).unwrap();
    super::representation(
        connection,
        RepresentationHeader::from_representation(import.representation()),
    )
    .unwrap();
    super::structure(
        connection,
        import.representation().id(),
        import.representation().content_structure(),
    )
    .unwrap();
    for resource in import.resources() {
        super::resource(connection, ResourceHeader::from_resource(resource)).unwrap();
    }
}

fn fixture() -> OriginalMediaImport {
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(-1), None, None);
    let resource = Resource::new(
        ResourceId::new(),
        Vec::new(),
        Some(FileFacts::new(123, None)),
    );
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        ContentStructure::single_resource(resource.id()),
        Vec::new(),
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource.id(),
        "file:///does-not-exist",
        None,
        LocatorAvailability::Offline,
    )
    .unwrap();
    OriginalMediaImport::new(asset, representation, vec![resource], vec![locator]).unwrap()
}
