//! Persistence coverage for compact and multi-resource content structures.

use postproject_core::{
    Asset, AssetId, ContentStructure, ErrorKind, FrameRange, ImageSequenceDescriptor, Locator,
    LocatorAvailability, LocatorId, OriginalMediaImport, RationalRate, Representation,
    RepresentationId, RepresentationImport, RepresentationKind, Resource, ResourceId,
    ResourceMember, ResourceRole, RevisionEventKind, SequenceNaming, Timestamp,
};
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one sequence verifies reopening and recording a renamed copy"
)]
fn sparse_image_sequence_reopens_without_per_frame_resources() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let production_path = directory.path().join("sequence.pproj");
    let now = Timestamp::from_unix_micros(1_000);
    let asset = Asset::new(AssetId::new(), now, Some("VFX plate".to_owned()), None);
    let resource_id = ResourceId::new();
    let frames = FrameRange::new(1_001, 1_100, 1).expect("valid frame range");
    let rate = RationalRate::new(24_000, 1_001).expect("valid rate");
    let sequence = ImageSequenceDescriptor::new(resource_id, frames, rate, vec![1_027, 1_042])
        .expect("valid sequence");
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        ContentStructure::image_sequence(sequence.clone()),
        Vec::new(),
    );
    let resource = Resource::new(resource_id, Vec::new(), None);
    let locator = Locator::new(
        LocatorId::new(),
        resource_id,
        "file:///production/plates/shot-a/",
        Some(now),
        LocatorAvailability::Online,
    )
    .expect("valid locator")
    .with_sequence_naming(SequenceNaming::new("plate.", ".exr", 4).expect("valid naming"));
    let import = OriginalMediaImport::new(
        asset,
        representation.clone(),
        vec![resource],
        vec![locator.clone()],
    )
    .expect("valid compound import");

    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let mut transaction = production.begin_transaction().expect("begin transaction");
    transaction.import_original(&import).expect("stage import");
    transaction.commit().expect("commit import");
    drop(transaction);
    drop(production);

    let reopened = SqliteProduction::open(&production_path).expect("reopen production");
    let stored = reopened
        .representations(import.asset().id())
        .expect("load representations");
    assert_eq!(stored, [representation]);
    assert_eq!(
        stored[0].content_structure().image_sequence_descriptor(),
        Some(&sequence)
    );
    assert_eq!(
        reopened
            .resources(stored[0].id())
            .expect("load sequence resources")
            .len(),
        1
    );
    assert_eq!(
        reopened.locators(resource_id).expect("load locators"),
        std::slice::from_ref(&locator)
    );
    drop(reopened);

    let connection = Connection::open(&production_path).expect("inspect database");
    let resource_rows: u32 = connection
        .query_row("SELECT count(*) FROM resources", [], |row| row.get(0))
        .expect("count resources");
    let exception_rows: u32 = connection
        .query_row(
            "SELECT count(*) FROM image_sequence_missing_frames",
            [],
            |row| row.get(0),
        )
        .expect("count sparse exceptions");
    assert_eq!(resource_rows, 1);
    assert_eq!(exception_rows, 2);
    drop(connection);

    // A renamed copy is recorded as a second locator, even at the same URI,
    // beside the locator that keeps the original names.
    let mut production = SqliteProduction::open(&production_path).expect("reopen for confirm");
    let renamed = Locator::new(
        LocatorId::new(),
        resource_id,
        "file:///production/plates/shot-a/",
        Some(now),
        LocatorAvailability::Online,
    )
    .expect("valid locator")
    .with_sequence_naming(SequenceNaming::new("plate-graded_", ".exr", 4).expect("naming"));
    let unnamed = Locator::new(
        LocatorId::new(),
        resource_id,
        "file:///backup/shot-a/",
        Some(now),
        LocatorAvailability::Online,
    )
    .expect("valid locator");
    let mut transaction = production.begin_transaction().expect("begin transaction");
    assert_eq!(
        transaction
            .add_locator(&unnamed)
            .expect_err("a sequence locator needs a naming")
            .kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        transaction
            .add_locator(
                &Locator::new(
                    LocatorId::new(),
                    resource_id,
                    locator.uri(),
                    None,
                    LocatorAvailability::Online,
                )
                .expect("valid locator")
                .with_sequence_naming(locator.sequence_naming().expect("naming").clone())
            )
            .expect_err("the same URI and naming is a duplicate")
            .kind(),
        ErrorKind::AlreadyExists
    );
    transaction.rollback().expect("roll back rejected locators");
    drop(transaction);
    let mut transaction = production.begin_transaction().expect("begin transaction");
    transaction
        .add_locator(&renamed)
        .expect("stage renamed locator");
    transaction.commit().expect("commit renamed locator");
    drop(transaction);
    let namings = production
        .locators(resource_id)
        .expect("load locators")
        .iter()
        .map(|locator| locator.sequence_naming().cloned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        namings,
        [
            Some(SequenceNaming::new("plate.", ".exr", 4).expect("naming")),
            Some(SequenceNaming::new("plate-graded_", ".exr", 4).expect("naming")),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn only_sequence_locators_carry_a_naming() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let production_path = directory.path().join("file.pproj");
    let resource_id = ResourceId::from_bytes([40; 16]);
    let import = compound_import(
        "File",
        ContentStructure::single_resource(resource_id),
        &[resource_id],
    );
    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let mut transaction = production.begin_transaction().expect("begin transaction");
    transaction.import_original(&import).expect("stage import");
    let named = Locator::new(
        LocatorId::new(),
        resource_id,
        "file:///elsewhere/clip.mov",
        None,
        LocatorAvailability::Online,
    )
    .expect("valid locator")
    .with_sequence_naming(SequenceNaming::new("clip_", ".mov", 4).expect("naming"));

    assert_eq!(
        transaction
            .add_locator(&named)
            .expect_err("a file locator has no naming")
            .kind(),
        ErrorKind::InvalidArgument
    );
}

#[test]
fn ordered_parts_and_package_membership_round_trip() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let production_path = directory.path().join("compound.pproj");
    let ordered_ids = [
        ResourceId::from_bytes([1; 16]),
        ResourceId::from_bytes([2; 16]),
        ResourceId::from_bytes([3; 16]),
    ];
    let part_role = ResourceRole::new("example.camera:essence-part").expect("valid role");
    let ordered = ContentStructure::ordered_parts(
        ordered_ids
            .iter()
            .map(|id| ResourceMember::new(*id, part_role.clone(), true))
            .collect(),
    )
    .expect("valid ordered parts");
    let ordered_import = compound_import("Spanned original", ordered, &ordered_ids);

    let package_ids = [
        ResourceId::from_bytes([4; 16]),
        ResourceId::from_bytes([5; 16]),
        ResourceId::from_bytes([6; 16]),
    ];
    let package = ContentStructure::package(vec![
        ResourceMember::new(
            package_ids[0],
            ResourceRole::new("org.postproject:essence").expect("valid role"),
            true,
        ),
        ResourceMember::new(
            package_ids[1],
            ResourceRole::new("example.camera:playlist").expect("valid role"),
            true,
        ),
        ResourceMember::new(
            package_ids[2],
            ResourceRole::new("org.postproject:thumbnail").expect("valid role"),
            false,
        ),
    ])
    .expect("valid package");
    let package_import = compound_import("Camera package", package, &package_ids);

    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let mut transaction = production.begin_transaction().expect("begin transaction");
    transaction
        .import_original(&ordered_import)
        .expect("stage ordered import");
    transaction
        .import_original(&package_import)
        .expect("stage package import");
    transaction.commit().expect("commit imports");
    drop(transaction);
    drop(production);

    let reopened = SqliteProduction::open(&production_path).expect("reopen production");
    for (import, expected_ids) in [
        (&ordered_import, ordered_ids.as_slice()),
        (&package_import, package_ids.as_slice()),
    ] {
        let stored = reopened
            .representations(import.asset().id())
            .expect("load representation");
        assert_eq!(stored, [import.representation().clone()]);
        assert_eq!(stored[0].content_structure().resource_ids(), expected_ids);
        let resources = reopened.resources(stored[0].id()).expect("load resources");
        assert_eq!(
            resources.iter().map(Resource::id).collect::<Vec<_>>(),
            expected_ids
        );
    }
}

#[test]
fn additional_representation_round_trips_and_is_journaled() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let production_path = directory.path().join("additional.pproj");
    let original = compound_import(
        "Source and proxy",
        ContentStructure::single_resource(ResourceId::from_bytes([10; 16])),
        &[ResourceId::from_bytes([10; 16])],
    );
    let asset_id = original.asset().id();
    let proxy_resource_id = ResourceId::from_bytes([11; 16]);
    let proxy = Representation::new(
        RepresentationId::from_bytes([12; 16]),
        asset_id,
        RepresentationKind::Proxy,
        ContentStructure::single_resource(proxy_resource_id),
        Vec::new(),
    );
    let proxy_locator = Locator::new(
        LocatorId::from_bytes([13; 16]),
        proxy_resource_id,
        "file:///production/proxy.mp4",
        Some(Timestamp::from_unix_micros(3_000)),
        LocatorAvailability::Online,
    )
    .expect("valid proxy locator");
    let proxy_import = RepresentationImport::new(
        proxy.clone(),
        vec![Resource::new(proxy_resource_id, Vec::new(), None)],
        vec![proxy_locator.clone()],
    )
    .expect("valid proxy import");

    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let mut transaction = production.begin_transaction().expect("begin transaction");
    transaction
        .import_original(&original)
        .expect("import original");
    transaction.commit().expect("commit original");
    drop(transaction);

    let mut transaction = production.begin_transaction().expect("begin transaction");
    transaction
        .add_representation(&proxy_import)
        .expect("add proxy");
    transaction.commit().expect("commit proxy");
    drop(transaction);
    drop(production);

    let reopened = SqliteProduction::open(&production_path).expect("reopen production");
    let representations = reopened
        .representations(asset_id)
        .expect("load representations");
    assert!(representations.contains(&proxy));
    assert_eq!(
        reopened
            .resources(proxy.id())
            .expect("load proxy resources"),
        proxy_import.resources()
    );
    assert_eq!(
        reopened
            .locators(proxy_resource_id)
            .expect("load proxy locators"),
        [proxy_locator]
    );
    let revision = reopened
        .latest_revision()
        .expect("load latest revision")
        .expect("proxy revision");
    let events = reopened
        .events_for_revision(revision.id())
        .expect("load proxy events");
    assert_eq!(events.len(), 4);
    assert!(matches!(
        events[0].kind(),
        RevisionEventKind::RepresentationAdded {
            representation_id,
            ..
        } if *representation_id == proxy.id()
    ));
    assert!(
        events
            .iter()
            .all(|event| !matches!(event.kind(), RevisionEventKind::AssetImported { .. }))
    );
}

#[test]
fn additional_representation_requires_an_existing_asset() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("missing.pproj"), None)
        .expect("create production");
    let resource_id = ResourceId::new();
    let representation = Representation::new(
        RepresentationId::new(),
        AssetId::new(),
        RepresentationKind::Derived,
        ContentStructure::single_resource(resource_id),
        Vec::new(),
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource_id,
        "file:///production/output.mov",
        None,
        LocatorAvailability::Online,
    )
    .expect("valid locator");
    let import = RepresentationImport::new(
        representation,
        vec![Resource::new(resource_id, Vec::new(), None)],
        vec![locator],
    )
    .expect("valid representation import");
    let mut transaction = production.begin_transaction().expect("begin transaction");

    assert_eq!(
        transaction
            .add_representation(&import)
            .expect_err("missing asset must fail")
            .kind(),
        ErrorKind::NotFound
    );
}

fn compound_import(
    name: &str,
    structure: ContentStructure,
    resource_ids: &[ResourceId],
) -> OriginalMediaImport {
    let now = Timestamp::from_unix_micros(2_000);
    let asset = Asset::new(AssetId::new(), now, Some(name.to_owned()), None);
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        structure,
        Vec::new(),
    );
    let resources = resource_ids
        .iter()
        .map(|id| Resource::new(*id, Vec::new(), None))
        .collect();
    let locators = resource_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            Locator::new(
                LocatorId::new(),
                *id,
                format!("file:///production/compound/member-{index}"),
                Some(now),
                LocatorAvailability::Online,
            )
            .expect("valid locator")
        })
        .collect();
    OriginalMediaImport::new(asset, representation, resources, locators)
        .expect("valid compound import")
}
