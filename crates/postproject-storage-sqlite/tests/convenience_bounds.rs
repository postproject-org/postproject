//! Complete-list helpers must refuse larger collections while pages stay usable.

use postproject_core::{
    ActivityId, AssetId, ErrorKind, QueryPageRequest, RepresentationId, ResourceId,
};
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::{Connection, params};

#[test]
fn larger_collections_remain_pageable_without_silent_truncation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large.pproj");
    let production = SqliteProduction::create(&path, None).unwrap();
    let asset = AssetId::new();
    let representation = RepresentationId::new();
    let resource = ResourceId::new();
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1001 {
        let id = AssetId::new();
        let id = if index == 0 { asset } else { id };
        transaction
            .execute(
                "INSERT INTO assets(id, created_at_micros) VALUES(?1, ?2)",
                params![id.as_bytes().as_slice(), index],
            )
            .unwrap();
        let id = RepresentationId::new();
        let id = if index == 0 { representation } else { id };
        transaction.execute("INSERT INTO representations(id, asset_id, kind, structure_kind) VALUES(?1, ?2, 0, ?3)",
            params![id.as_bytes().as_slice(), asset.as_bytes().as_slice(), if index == 0 { 2 } else { 0 }]).unwrap();
        let member = if index == 0 {
            resource
        } else {
            ResourceId::new()
        };
        transaction
            .execute(
                "INSERT INTO resources(id) VALUES(?1)",
                [member.as_bytes().as_slice()],
            )
            .unwrap();
        transaction.execute("INSERT INTO representation_resources(representation_id, resource_id, position, role, required) VALUES(?1, ?2, ?3, 'example:part', 1)",
            params![representation.as_bytes().as_slice(), member.as_bytes().as_slice(), index]).unwrap();
        if index != 0 {
            transaction.execute("INSERT INTO representation_resources(representation_id, resource_id, position, required) VALUES(?1, ?2, 0, 1)",
                params![id.as_bytes().as_slice(), member.as_bytes().as_slice()]).unwrap();
        }
        transaction
            .execute(
                "INSERT INTO locators(id, resource_id, uri, availability) VALUES(?1, ?2, ?3, 0)",
                params![
                    id.as_bytes().as_slice(),
                    resource.as_bytes().as_slice(),
                    format!("file:///media/{index}.mov")
                ],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    let page = QueryPageRequest::new(1000, None).unwrap();
    assert_eq!(
        production.assets().unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    let error = production.representations(asset).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Unsupported, "{error}");
    assert_eq!(
        production.resources(representation).unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    assert_eq!(
        production.locators(resource).unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    let assets = production.assets_page(&page).unwrap();
    let representations = production.representations_page(asset, &page).unwrap();
    let resources = production.resources_page(representation, &page).unwrap();
    let locators = production.locators_page(resource, &page).unwrap();
    for (count, cursor) in [
        (assets.items().len(), assets.next_cursor()),
        (representations.items().len(), representations.next_cursor()),
        (resources.items().len(), resources.next_cursor()),
        (locators.items().len(), locators.next_cursor()),
    ] {
        assert_eq!(count, 1000);
        assert!(cursor.is_some());
    }
    let last = production
        .assets_page(&QueryPageRequest::new(1000, assets.next_cursor().cloned()).unwrap())
        .unwrap();
    assert_eq!(last.items().len(), 1);
    assert!(last.next_cursor().is_none());
}

#[test]
fn resource_pages_share_their_fingerprint_payload_budget() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large-fingerprints.pproj");
    let production = SqliteProduction::create(&path, None).unwrap();
    let asset = AssetId::new();
    let representation = RepresentationId::new();
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    transaction
        .execute(
            "INSERT INTO assets(id, created_at_micros) VALUES(?1, 0)",
            [asset.as_bytes().as_slice()],
        )
        .unwrap();
    transaction
        .execute(
            "INSERT INTO representations(id, asset_id, kind, structure_kind) VALUES(?1, ?2, 0, 2)",
            params![
                representation.as_bytes().as_slice(),
                asset.as_bytes().as_slice()
            ],
        )
        .unwrap();
    for position in 0..8 {
        let resource = ResourceId::new();
        transaction
            .execute(
                "INSERT INTO resources(id) VALUES(?1)",
                [resource.as_bytes().as_slice()],
            )
            .unwrap();
        transaction.execute("INSERT INTO representation_resources(representation_id, resource_id, position, role, required) VALUES(?1, ?2, ?3, 'example:part', 1)",
            params![representation.as_bytes().as_slice(), resource.as_bytes().as_slice(), position]).unwrap();
        transaction.execute("INSERT INTO resource_fingerprints(resource_id, algorithm, algorithm_version, value) VALUES(?1, 'example-opaque', 1, zeroblob(9*1024*1024))",
            [resource.as_bytes().as_slice()]).unwrap();
    }
    transaction.commit().unwrap();
    let before = production.latest_revision().unwrap();
    let error = production
        .resources_page(representation, &QueryPageRequest::new(8, None).unwrap())
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Unsupported, "{error}");
    let mut cursor = None;
    let mut count = 0;
    loop {
        let page = production
            .resources_page(representation, &QueryPageRequest::new(1, cursor).unwrap())
            .unwrap();
        assert_eq!(page.items().len(), 1);
        assert_eq!(
            page.items()[0].fingerprints()[0].value().len(),
            9 * 1024 * 1024
        );
        count += 1;
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(count, 8);
    assert_eq!(production.latest_revision().unwrap(), before);
}

#[test]
fn activity_pages_share_their_edge_snapshot_budget() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large-snapshots.pproj");
    let production = SqliteProduction::create(&path, None).unwrap();
    let asset = AssetId::new();
    let representation = RepresentationId::new();
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    transaction
        .execute(
            "INSERT INTO assets(id, created_at_micros) VALUES(?1, 0)",
            [asset.as_bytes().as_slice()],
        )
        .unwrap();
    transaction
        .execute(
            "INSERT INTO representations(id, asset_id, kind, structure_kind) VALUES(?1, ?2, 0, 0)",
            params![
                representation.as_bytes().as_slice(),
                asset.as_bytes().as_slice()
            ],
        )
        .unwrap();
    for edge in 1..=2 {
        let activity = ActivityId::new();
        transaction
            .execute(
                "INSERT INTO activities(id, kind) VALUES(?1, 'example:render')",
                [activity.as_bytes().as_slice()],
            )
            .unwrap();
        transaction.execute("INSERT INTO activity_inputs(id, activity_id, representation_id, snapshot_revision_sequence) VALUES(?1, ?2, ?3, 1)",
            params![edge, activity.as_bytes().as_slice(), representation.as_bytes().as_slice()]).unwrap();
        let output = RepresentationId::new();
        transaction.execute("INSERT INTO representations(id, asset_id, kind, structure_kind) VALUES(?1, ?2, 0, 0)",
            params![output.as_bytes().as_slice(), asset.as_bytes().as_slice()]).unwrap();
        transaction
            .execute(
                "INSERT INTO activity_outputs(activity_id, representation_id) VALUES(?1, ?2)",
                params![activity.as_bytes().as_slice(), output.as_bytes().as_slice()],
            )
            .unwrap();
        for version in 1..=4 {
            transaction.execute("INSERT INTO activity_input_fingerprint_snapshots(activity_input_id, algorithm, algorithm_version, value) VALUES(?1, 'example-opaque', ?2, zeroblob(9*1024*1024))",
                params![edge, version]).unwrap();
        }
    }
    transaction.commit().unwrap();
    let before = production.latest_revision().unwrap();
    assert_eq!(
        production.activities().unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    let error = production
        .activities_consuming_page(representation, &QueryPageRequest::new(2, None).unwrap())
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Unsupported, "{error}");
    let first = production
        .activities_consuming_page(representation, &QueryPageRequest::new(1, None).unwrap())
        .unwrap();
    let second = production
        .activities_consuming_page(
            representation,
            &QueryPageRequest::new(1, first.next_cursor().cloned()).unwrap(),
        )
        .unwrap();
    for page in [&first, &second] {
        assert_eq!(page.items().len(), 1);
        let fingerprints = page.items()[0].inputs()[0]
            .snapshot()
            .unwrap()
            .fingerprints();
        assert_eq!(fingerprints.len(), 4);
        assert!(
            fingerprints
                .iter()
                .all(|value| value.value().len() == 9 * 1024 * 1024)
        );
    }
    assert!(second.next_cursor().is_none());
    assert_eq!(production.latest_revision().unwrap(), before);
}
