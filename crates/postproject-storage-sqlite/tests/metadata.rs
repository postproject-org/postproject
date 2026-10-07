//! Integration coverage for typed metadata persistence and mutation semantics.

use std::fs;

use postproject_core::{
    ActivityId, AssetId, ErrorKind, MetadataField, MetadataProperty, MetadataQuery, MetadataValue,
    ObjectRef, PropertyId, QueryPageRequest, VocabularyId,
};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;
use tempfile::tempdir;

fn property(vocabulary: &str, name: &str) -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new(vocabulary).expect("valid vocabulary"),
        PropertyId::new(name).expect("valid property"),
    )
}

fn structured_contact() -> MetadataValue {
    MetadataValue::structure(vec![
        MetadataField::new(
            PropertyId::new("name").unwrap(),
            MetadataValue::language_string("Kameraabteilung", "de-DE").unwrap(),
        ),
        MetadataField::new(
            PropertyId::new("url").unwrap(),
            MetadataValue::uri("https://example.com/crew/camera").unwrap(),
        ),
    ])
    .unwrap()
}

#[test]
fn payload_budget_rejects_large_materialization_but_smaller_pages_preserve_values() {
    let directory = tempdir().unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("large.pproj"), None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let property = property("com.example.opaque", "payload");
    let value = MetadataValue::bytes(vec![0x8a; 9 * 1024 * 1024]).unwrap();
    let mut edit = production.begin_transaction().unwrap();
    for _ in 0..8 {
        edit.add_metadata_value(target, &property, &value).unwrap();
    }
    let receipt = edit.commit().unwrap();
    drop(edit);
    assert_eq!(
        production.metadata(target).unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    assert_eq!(
        production
            .metadata_values(target, &property)
            .unwrap_err()
            .kind(),
        ErrorKind::Unsupported
    );
    let query = MetadataQuery::new(property, None).unwrap();
    assert_eq!(
        production
            .metadata_query(&query, &QueryPageRequest::new(8, None).unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::Unsupported
    );
    let mut cursor = None;
    let mut count = 0;
    loop {
        let page = production
            .metadata_query(&query, &QueryPageRequest::new(1, cursor).unwrap())
            .unwrap();
        assert_eq!(page.items().len(), 1);
        assert_eq!(page.items()[0].assertion().value(), &value);
        count += page.items().len();
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(count, 8);
    assert_eq!(
        production.latest_revision().unwrap().unwrap().id(),
        receipt.revision().unwrap().id()
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "round-trip, queries and read-bound replacement share one fixture"
)]
fn repeated_and_structured_metadata_round_trip_and_query() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let media_path = directory.path().join("A001.mov");
    fs::write(&media_path, b"fixture media bytes").expect("write fixture media");
    let prepared = prepare_original_media(&media_path, None, None).expect("prepare import");
    let asset = ObjectRef::Asset(prepared.asset().id());
    let representation = ObjectRef::Representation(prepared.representation().id());
    let resource = ObjectRef::Resource(prepared.resources()[0].id());
    let keywords = property("http://iptc.org/std/videometadatahub/1.0", "keywords");
    let contact = property(
        "http://iptc.org/std/videometadatahub/1.0",
        "creatorContactInfo",
    );
    let application_note = property("com.example.editor/metadata", "note");
    let structured_contact = structured_contact();

    let mut production = SqliteProduction::create(&production_path, None).unwrap();
    let production_target = ObjectRef::Production(production.production().id());
    {
        let mut transaction = production.begin_transaction().expect("begin transaction");
        transaction
            .import_original(&prepared)
            .expect("import media");
        transaction
            .add_metadata_value(
                asset,
                &keywords,
                &MetadataValue::string("interview").unwrap(),
            )
            .expect("add first keyword");
        transaction
            .add_metadata_value(asset, &keywords, &MetadataValue::string("night").unwrap())
            .expect("add second keyword");
        transaction
            .add_metadata_value(asset, &contact, &structured_contact)
            .expect("add structured contact");
        transaction
            .add_metadata_value(
                representation,
                &application_note,
                &MetadataValue::string("camera original").unwrap(),
            )
            .expect("add representation metadata");
        transaction
            .add_metadata_value(
                resource,
                &application_note,
                &MetadataValue::string("storage-specific note").unwrap(),
            )
            .expect("add resource metadata");
        transaction
            .add_metadata_value(
                production_target,
                &application_note,
                &MetadataValue::string("production note").unwrap(),
            )
            .expect("add production metadata");
        transaction.commit().expect("commit metadata");
    }
    drop(production);

    let mut reopened = SqliteProduction::open(&production_path).expect("reopen production");
    assert_eq!(
        reopened.metadata_values(asset, &keywords).unwrap(),
        [
            MetadataValue::string("interview").unwrap(),
            MetadataValue::string("night").unwrap(),
        ]
    );
    assert_eq!(
        reopened.metadata_values(asset, &contact).unwrap(),
        std::slice::from_ref(&structured_contact)
    );
    assert_eq!(reopened.metadata(asset).unwrap().len(), 3);
    let matches = reopened
        .query_by_metadata_property(&application_note)
        .expect("query application property");
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].target(), production_target);
    assert_eq!(matches[1].target(), representation);
    assert_eq!(matches[2].target(), resource);

    {
        let base = reopened.read_session().unwrap().decision_base();
        let mut transaction = reopened.begin_edit(base).unwrap();
        transaction
            .replace_metadata_values(
                asset,
                &keywords,
                &[
                    MetadataValue::string("replacement-one").unwrap(),
                    MetadataValue::string("replacement-two").unwrap(),
                ],
            )
            .expect("replace repeated values");
        transaction.commit().unwrap();
    }
    assert_eq!(
        reopened.metadata_values(asset, &keywords).unwrap(),
        [
            MetadataValue::string("replacement-one").unwrap(),
            MetadataValue::string("replacement-two").unwrap(),
        ]
    );
}

#[test]
fn metadata_mutations_are_atomic_and_validate_targets() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let target = ObjectRef::Production(production.production().id());
    let title = property("https://example.com/vocabulary", "title");
    let value = MetadataValue::string("Documentary").unwrap();

    {
        let mut transaction = production.begin_transaction().unwrap();
        transaction
            .add_metadata_value(target, &title, &value)
            .expect("stage metadata");
        transaction.rollback().expect("roll back metadata");
    }
    assert_eq!(production.metadata_values(target, &title).unwrap(), []);

    {
        let mut transaction = production.begin_transaction().unwrap();
        transaction
            .add_metadata_value(target, &title, &value)
            .expect("stage metadata");
        transaction.commit().expect("commit metadata");
    }
    {
        let base = production.read_session().unwrap().decision_base();
        let mut transaction = production.begin_edit(base).unwrap();
        transaction
            .remove_metadata_property(target, &title)
            .expect("stage removal");
        transaction.rollback().expect("roll back removal");
    }
    assert_eq!(
        production.metadata_values(target, &title).unwrap(),
        std::slice::from_ref(&value)
    );

    let mut transaction = production.begin_transaction().unwrap();
    assert_eq!(
        transaction
            .add_metadata_value(ObjectRef::Asset(AssetId::new()), &title, &value)
            .expect_err("missing target")
            .kind(),
        ErrorKind::NotFound
    );
    assert_eq!(
        transaction
            .add_metadata_value(ObjectRef::Activity(ActivityId::new()), &title, &value)
            .expect_err("missing activity target")
            .kind(),
        ErrorKind::NotFound
    );
    transaction.rollback().unwrap();
}

#[test]
fn malformed_persisted_metadata_fails_safely() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let target = ObjectRef::Production(production.production().id());
    let title = property("https://example.com/vocabulary", "title");
    {
        let mut transaction = production.begin_transaction().unwrap();
        transaction
            .add_metadata_value(target, &title, &MetadataValue::string("valid").unwrap())
            .unwrap();
        transaction.commit().unwrap();
    }
    drop(production);

    let connection = Connection::open(&production_path).expect("open raw database");
    connection
        .execute(
            "UPDATE metadata_assertions SET encoded_value = X'50504D5601FF'",
            [],
        )
        .expect("corrupt encoded value");
    drop(connection);

    let reopened = SqliteProduction::open(&production_path).expect("reopen production");
    assert_eq!(
        reopened
            .metadata(target)
            .expect_err("corrupt metadata must fail")
            .kind(),
        ErrorKind::Storage
    );
}
