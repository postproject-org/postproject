//! Missing retained capture cannot silently turn later writes into partial history.

use postproject_core::{
    ErrorKind, MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId,
};
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;

#[test]
fn changing_commit_with_missing_predecessor_rolls_back_every_new_portable_fact() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(source.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:gap").unwrap(),
        PropertyId::new("value").unwrap(),
    );
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(1))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    // An incomplete older development history or damaged current store must
    // require explicit recovery rather than manufacture a complete successor.
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM exchange_records WHERE sequence = 1", [])
        .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(2))
        .unwrap();
    assert_eq!(edit.commit().unwrap_err().kind(), ErrorKind::Storage);
    drop(edit);
    assert_eq!(
        source.metadata_values(target, &property).unwrap(),
        [MetadataValue::i64(1)]
    );
    assert_eq!(source.changes_since(0, 10).unwrap().len(), 1);
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM exchange_records", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM exchange_effect_fragments",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(source.exchange_floor().unwrap().sequence(), 0);
    assert!(source.exchange_head().is_err());
}
