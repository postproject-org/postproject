//! Authored metadata capture; other families and complete replay remain pending.

use postproject_core::{
    ErrorKind, MetadataProperty, MetadataValue, ObjectRef, PropertyId, TransactionState,
    VocabularyId,
};
use postproject_protocol::{Document, Limits, MetadataChange, MetadataEffect};
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:exchange:test").unwrap(),
        PropertyId::new("ordered").unwrap(),
    )
}

fn effects(path: &std::path::Path) -> Vec<MetadataEffect> {
    let connection = Connection::open(path).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT effect_position, payload FROM exchange_effect_fragments
         ORDER BY effect_position, fragment_position",
        )
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .unwrap();
    let mut documents = Vec::<Vec<u8>>::new();
    for row in rows {
        let (position, fragment) = row.unwrap();
        let position = usize::try_from(position).unwrap();
        if position == documents.len() {
            documents.push(Vec::new());
        }
        documents[position].extend(fragment);
    }
    documents
        .iter()
        .map(|bytes| {
            MetadataEffect::from_document(&Document::parse(bytes, Limits::default()).unwrap())
                .unwrap()
        })
        .collect()
}

#[test]
fn native_same_key_edits_retain_order_and_assigned_positions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let base = production.read_session().unwrap().decision_base();
    let mut edit = production.begin_edit(base).unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(11))
        .unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(12))
        .unwrap();
    edit.replace_metadata_values(target, &property(), &[MetadataValue::i64(13)])
        .unwrap();
    edit.remove_metadata_property(target, &property()).unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(14))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let captured = effects(&path);
    assert_eq!(captured.len(), 5);
    assert!(matches!(
        captured[0].change(),
        MetadataChange::Appended {
            position: 0,
            value
        } if value == &MetadataValue::i64(11)
    ));
    assert!(matches!(
        captured[1].change(),
        MetadataChange::Appended {
            position: 1,
            value
        } if value == &MetadataValue::i64(12)
    ));
    assert!(
        matches!(captured[2].change(), MetadataChange::Replaced(values) if values == [MetadataValue::i64(13)])
    );
    assert!(matches!(captured[3].change(), MetadataChange::Removed));
    assert!(matches!(
        captured[4].change(),
        MetadataChange::Appended {
            position: 0,
            value
        } if value == &MetadataValue::i64(14)
    ));
    assert_eq!(
        production.metadata_values(target, &property()).unwrap(),
        [MetadataValue::i64(14)]
    );
    assert_eq!(production.changes_since(0, 10).unwrap().len(), 1);
}

#[test]
fn effect_persistence_failure_rolls_back_domain_revision_and_fragments() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_effect BEFORE INSERT ON exchange_effect_fragments
         WHEN NEW.effect_position = 1 BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .unwrap();
    let target = ObjectRef::Production(production.production().id());
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(1))
        .unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(2))
        .unwrap();
    assert_eq!(edit.commit().unwrap_err().kind(), ErrorKind::Storage);
    assert_eq!(edit.state(), TransactionState::RolledBack);
    assert!(edit.commit().is_err());
    drop(edit);
    assert!(effects(&path).is_empty());
    assert!(
        production
            .metadata_values(target, &property())
            .unwrap()
            .is_empty()
    );
    assert!(production.changes_since(0, 10).unwrap().is_empty());
}

#[test]
fn no_op_and_rollback_leave_no_effects() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let base = production.read_session().unwrap().decision_base();
    let mut edit = production.begin_edit(base).unwrap();
    edit.replace_metadata_values(target, &property(), &[])
        .unwrap();
    assert!(edit.commit().unwrap().revision().is_none());
    drop(edit);
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(1))
        .unwrap();
    edit.rollback().unwrap();
    drop(edit);
    assert!(effects(&path).is_empty());
}

#[test]
fn legal_large_values_and_native_command_counts_have_no_proposal_cap() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let value = MetadataValue::bytes(vec![0xff; 15 * 1024 * 1024]).unwrap();
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &value)
        .unwrap();
    for _ in 0..1001 {
        edit.add_metadata_value(target, &property(), &MetadataValue::i64(1))
            .unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    let captured = effects(&path);
    assert_eq!(captured.len(), 1002);
    assert!(
        matches!(captured[0].change(), MetadataChange::Appended { position: 0, value: decoded } if decoded == &value)
    );
}
