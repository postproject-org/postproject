//! Metadata records are captured atomically inside their original native edit.

use std::path::Path;

use postproject_core::{
    ErrorKind, MetadataProperty, MetadataValue, ObjectRef, PropertyId, TransactionState,
    VocabularyId,
};
use postproject_protocol::{
    Document, Limits, MetadataEffectStart, MetadataOperation, RecordManifest, decode_event,
};
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:record:test").unwrap(),
        PropertyId::new("ordered").unwrap(),
    )
}

fn record(path: &Path, sequence: i64) -> (RecordManifest, Vec<Document>) {
    let production = SqliteProduction::open(path).unwrap();
    let mut reader = production
        .record_reader(u64::try_from(sequence).unwrap())
        .unwrap();
    let manifest = reader.manifest().clone();
    let mut bytes = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        bytes.extend_from_slice(chunk.payload());
    }
    assert!(reader.is_complete());
    let mut frames = Vec::new();
    let mut remaining = bytes.as_slice();
    while !remaining.is_empty() {
        let size = usize::try_from(u64::from_be_bytes(remaining[..8].try_into().unwrap())).unwrap();
        frames.push(Document::parse(&remaining[8..8 + size], Limits::default()).unwrap());
        remaining = &remaining[8 + size..];
    }
    (manifest, frames)
}

#[test]
fn native_records_keep_same_key_history_receipts_events_and_contiguous_heads() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let base = production.read_session().unwrap().decision_base();
    let mut edit = production.begin_edit(base).unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(11))
        .unwrap();
    edit.replace_metadata_values(
        target,
        &property(),
        &[MetadataValue::i64(12), MetadataValue::i64(12)],
    )
    .unwrap();
    edit.remove_metadata_property(target, &property()).unwrap();
    let receipt = edit.commit().unwrap();
    drop(edit);
    let (first, frames) = record(&path, 1);
    assert_eq!(Some(first.revision()), receipt.revision());
    assert_eq!(first.predecessor(), production.exchange_floor().unwrap());
    assert_eq!(first.effect_count(), 3);
    assert_eq!(first.event_count(), 3);
    assert_eq!(
        MetadataEffectStart::from_document(&frames[0])
            .unwrap()
            .operation(),
        MetadataOperation::Appended(0)
    );
    assert_eq!(
        MetadataEffectStart::decode_value(&frames[1]).unwrap(),
        MetadataValue::i64(11)
    );
    assert_eq!(
        MetadataEffectStart::from_document(&frames[2])
            .unwrap()
            .value_count(),
        2
    );
    assert_eq!(
        MetadataEffectStart::decode_value(&frames[3]).unwrap(),
        MetadataValue::i64(12)
    );
    assert_eq!(
        MetadataEffectStart::decode_value(&frames[4]).unwrap(),
        MetadataValue::i64(12)
    );
    assert_eq!(
        MetadataEffectStart::from_document(&frames[5])
            .unwrap()
            .operation(),
        MetadataOperation::Removed
    );
    for (position, frame) in frames[6..].iter().enumerate() {
        let event = decode_event(frame).unwrap();
        assert_eq!(event.revision_id(), first.revision().id());
        assert_eq!(event.position(), u32::try_from(position).unwrap());
    }
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(13))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    drop(production);
    assert_eq!(record(&path, 2).0.predecessor(), first.head().unwrap());
}

#[test]
fn chunk_or_manifest_failure_rolls_back_domain_revision_and_record_together() {
    for table in ["exchange_record_chunks", "exchange_records"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("authority.pproj");
        let mut production = SqliteProduction::create(&path, None).unwrap();
        Connection::open(&path).unwrap().execute_batch(&format!("CREATE TRIGGER reject_record BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
        let target = ObjectRef::Production(production.production().id());
        let mut edit = production.begin_transaction().unwrap();
        edit.add_metadata_value(target, &property(), &MetadataValue::i64(1))
            .unwrap();
        assert_eq!(edit.commit().unwrap_err().kind(), ErrorKind::Storage);
        assert_eq!(edit.state(), TransactionState::RolledBack);
        drop(edit);
        assert_eq!(
            production.metadata_values(target, &property()).unwrap(),
            Vec::<MetadataValue>::new()
        );
        assert!(production.latest_revision().unwrap().is_none());
        let connection = Connection::open(&path).unwrap();
        for table in [
            "exchange_records",
            "exchange_record_chunks",
            "exchange_effect_fragments",
        ] {
            assert_eq!(
                connection
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    }
}

#[test]
fn public_reader_pins_chunks_and_finishes_without_collecting_the_body() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    assert_eq!(
        production.exchange_head().unwrap(),
        production.exchange_floor().unwrap()
    );
    assert!(production.record_reader(0).is_err());
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(
        target,
        &property(),
        &MetadataValue::bytes(vec![7; 1024 * 1024]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let head = production.exchange_head().unwrap();
    let mut reader = production.record_reader(1).unwrap();
    assert!(reader.manifest().chunks().count() > 1);
    assert!(!reader.is_complete());
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(2))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert_eq!(reader.manifest().head().unwrap(), head);
    let mut count = 0;
    while reader.next_chunk().unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, reader.manifest().chunks().count());
    assert!(reader.is_complete());
    assert!(reader.next_chunk().unwrap().is_none());
    assert_eq!(
        production
            .record_reader(2)
            .unwrap()
            .manifest()
            .predecessor(),
        head
    );
    assert!(production.record_reader(3).is_err());
    assert!(production.record_reader(u64::MAX).is_err());
}

#[test]
fn missing_or_corrupt_chunks_make_public_readers_terminal() {
    for missing in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("authority.pproj");
        let mut production = SqliteProduction::create(&path, None).unwrap();
        let target = ObjectRef::Production(production.production().id());
        let mut edit = production.begin_transaction().unwrap();
        edit.add_metadata_value(target, &property(), &MetadataValue::i64(1))
            .unwrap();
        edit.commit().unwrap();
        drop(edit);
        Connection::open(&path)
            .unwrap()
            .execute_batch(if missing {
                "DELETE FROM exchange_record_chunks;"
            } else {
                "UPDATE exchange_record_chunks SET document = X'7B7D';"
            })
            .unwrap();
        let mut reader = production.record_reader(1).unwrap();
        assert!(reader.next_chunk().is_err());
        assert!(!reader.is_complete());
        assert!(reader.next_chunk().is_err());
    }
}

#[test]
fn uncaptured_families_never_advertise_a_complete_replay_head() {
    use postproject_core::{MediaRoot, MediaRootId};
    use postproject_protocol::FailureKind;
    use postproject_storage_sqlite::ExchangeError;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let mut edit = production.begin_transaction().unwrap();
    edit.add_media_root(MediaRoot::new(MediaRootId::new(), "rushes", None, None, 0, true).unwrap())
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert!(
        matches!(production.exchange_head(), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::HistoryGap)
    );
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(1))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert!(
        matches!(production.record_reader(2), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::HistoryGap)
    );
    assert_eq!(
        production.metadata_values(target, &property()).unwrap(),
        [MetadataValue::i64(1)]
    );
    assert_eq!(production.exchange_floor().unwrap().sequence(), 0);
}
