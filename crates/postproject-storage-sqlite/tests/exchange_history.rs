//! Persistent identities and honest migration floors before complete capture.

use postproject_core::{
    ErrorKind, MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId,
};
use postproject_protocol::{Position, ProtocolBase};
use postproject_storage_sqlite::{CURRENT_SCHEMA_VERSION, SqliteProduction};
use rusqlite::Connection;

#[test]
fn creation_reopen_and_read_view_keep_the_same_genesis_anchor() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let production = SqliteProduction::create(&path, Some("Authority".into())).unwrap();
    let scope = production.exchange_scope().unwrap();
    let anchor = production.exchange_floor().unwrap();
    assert_eq!(anchor.scope(), scope);
    assert_eq!(anchor.sequence(), 0);
    let session = production.read_session().unwrap();
    assert_eq!(
        Position::anchor(ProtocolBase::new(scope, session.decision_base()).unwrap()).unwrap(),
        anchor
    );
    let reader = session.into_read_only();
    assert_eq!(reader.exchange_floor().unwrap(), anchor);
    drop(reader);
    drop(production);
    let reopened = SqliteProduction::open(&path).unwrap();
    assert_eq!(reopened.exchange_scope().unwrap(), scope);
    assert_eq!(reopened.exchange_floor().unwrap(), anchor);
    let another = SqliteProduction::create(directory.path().join("another.pproj"), None).unwrap();
    assert_ne!(another.exchange_scope().unwrap().history(), scope.history());
}

#[test]
fn schema_19_migration_anchors_at_retained_head_without_fabricating_effects() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:legacy").unwrap(),
        PropertyId::new("fact").unwrap(),
    );
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(
        target,
        &property,
        &MetadataValue::string("retained").unwrap(),
    )
    .unwrap();
    let receipt = edit.commit().unwrap();
    drop(edit);
    drop(production);
    // Remove development exchange relations to recreate schema 19 with its
    // retained native revision/domain facts.
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("DROP TABLE exchange_record_chunks; DROP TABLE exchange_records; DROP TABLE exchange_outcomes; DROP TABLE exchange_effect_fragments; DROP TABLE exchange_history; DELETE FROM schema_migrations WHERE version >= 20; UPDATE productions SET schema_version = 19; PRAGMA user_version = 19;").unwrap();
    drop(connection);
    let production = SqliteProduction::open(&path).unwrap();
    let floor = production.exchange_floor().unwrap();
    assert_eq!(
        floor.revision(),
        receipt.revision().map(postproject_core::Revision::id)
    );
    assert_eq!(floor.sequence(), 1);
    assert_eq!(
        production.metadata_values(target, &property).unwrap(),
        vec![MetadataValue::string("retained").unwrap()]
    );
    assert_eq!(
        production.production().schema_version(),
        CURRENT_SCHEMA_VERSION
    );
    assert_eq!(production.changes_since(0, 10).unwrap().len(), 1);
    drop(production);
    assert_eq!(
        SqliteProduction::open(&path)
            .unwrap()
            .exchange_floor()
            .unwrap(),
        floor
    );
}

#[test]
fn corrupted_anchor_and_forged_retained_floor_are_storage_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("corrupted.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:anchor").unwrap(),
        PropertyId::new("fact").unwrap(),
    );
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::string("fact").unwrap())
        .unwrap();
    let receipt = edit.commit().unwrap();
    drop(edit);
    let original = production.exchange_floor().unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE exchange_history SET anchor_digest = zeroblob(32)",
            [],
        )
        .unwrap();
    assert_eq!(
        production.exchange_floor().unwrap_err().kind(),
        ErrorKind::Storage
    );
    // A digest consistent with a forged sequence still cannot authorize a floor
    // whose retained revision has a different durable sequence.
    let revision = receipt.revision().unwrap().id();
    let forged = Position::anchor(
        ProtocolBase::new(
            original.scope(),
            postproject_core::DecisionBase::new(production.production().id(), Some(revision), 2)
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    connection.execute(
        "UPDATE exchange_history SET floor_revision_id = ?1, floor_sequence = 2, anchor_digest = ?2",
        rusqlite::params![revision.as_bytes().as_slice(), forged.digest().as_bytes().as_slice()],
    ).unwrap();
    assert_eq!(
        production.exchange_floor().unwrap_err().kind(),
        ErrorKind::Storage
    );
    drop(production);
    let reopened = SqliteProduction::open(&path).unwrap();
    assert_eq!(
        reopened.exchange_floor().unwrap_err().kind(),
        ErrorKind::Storage
    );
}
