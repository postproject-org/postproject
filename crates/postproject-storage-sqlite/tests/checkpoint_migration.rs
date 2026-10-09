//! Retained observations and state survive honest released-schema replay floors.

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};
use postproject_protocol::{CheckpointChunk, CheckpointManifest, FailureKind};
use postproject_storage_sqlite::{CheckpointLimits, ExchangeError, ReplayLimits, SqliteProduction};

fn export(source: &SqliteProduction) -> (CheckpointManifest, Vec<CheckpointChunk>) {
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    (manifest, chunks)
}

#[test]
fn migrated_baseline_and_unknown_prefix_survive_checkpoint_and_suffix() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(source.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:legacy").unwrap(),
        PropertyId::new("Exact").unwrap(),
    );
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(1))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    drop(source);
    let connection = rusqlite::Connection::open(&path).unwrap();
    // Recreate the released schema-19 layout with its native facts/history.
    connection.execute_batch("DROP TABLE exchange_record_chunks; DROP TABLE exchange_records; DROP TABLE exchange_outcomes; DROP TABLE exchange_effect_fragments; DROP TABLE exchange_history; DELETE FROM schema_migrations WHERE version >= 20; UPDATE productions SET schema_version = 19; PRAGMA user_version = 19;").unwrap();
    drop(connection);
    let mut source = SqliteProduction::open(&path).unwrap();
    let floor = source.exchange_floor().unwrap();
    let (initial, chunks) = export(&source);
    assert_eq!(initial.head(), floor);
    assert_eq!(floor.sequence(), 1);
    let mut mirror = SqliteProduction::import_checkpoint(
        directory.path().join("mirror.pproj"),
        &initial,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(mirror.exchange_head().unwrap(), floor);
    assert_eq!(
        mirror.metadata_values(target, &property).unwrap(),
        vec![MetadataValue::i64(1)]
    );
    assert!(
        matches!(mirror.record_reader(1), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::HistoryGap)
    );
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(2))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let (later, chunks) = export(&source);
    let later_mirror = SqliteProduction::import_checkpoint(
        directory.path().join("later.pproj"),
        &later,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    let mut reader = source.record_reader(2).unwrap();
    let manifest = reader.manifest().clone();
    let mut chunks = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        chunks.push(chunk);
    }
    mirror
        .apply_record(
            &manifest,
            chunks.into_iter().map(Ok),
            ReplayLimits::default(),
        )
        .unwrap();
    for production in [&mirror, &later_mirror] {
        assert_eq!(
            production.exchange_head().unwrap(),
            source.exchange_head().unwrap()
        );
        assert_eq!(production.exchange_floor().unwrap(), floor);
        assert_eq!(
            production.metadata_values(target, &property).unwrap(),
            vec![MetadataValue::i64(1), MetadataValue::i64(2)]
        );
        assert_eq!(
            production.changes_since(0, 10).unwrap(),
            source.changes_since(0, 10).unwrap()
        );
    }
}

#[test]
fn migrated_roots_keep_unknown_header_facts_and_recorded_removal_boundaries() {
    use postproject_core::{MediaRoot, MediaRootId};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy-roots.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let retained = MediaRoot::new(
        MediaRootId::new(),
        "Essence",
        Some("Exact".into()),
        None,
        1,
        true,
    )
    .unwrap();
    let removed = MediaRoot::new(MediaRootId::new(), "Old", None, None, 2, true).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.add_media_root(retained.clone()).unwrap();
    edit.add_media_root(removed.clone()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    drop(source);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("DROP TABLE exchange_record_chunks; DROP TABLE exchange_records; DROP TABLE exchange_outcomes; DROP TABLE exchange_effect_fragments; DROP TABLE exchange_history; DELETE FROM schema_migrations WHERE version >= 20; UPDATE productions SET schema_version = 19; PRAGMA user_version = 19;").unwrap();
    drop(connection);
    let mut source = SqliteProduction::open(&path).unwrap();
    let floor = source.exchange_floor().unwrap();
    let (initial, chunks) = export(&source);
    let mut mirror = SqliteProduction::import_checkpoint(
        directory.path().join("root-mirror.pproj"),
        &initial,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(mirror.media_roots().unwrap(), source.media_roots().unwrap());
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.set_media_root_enabled(retained.id(), false).unwrap();
    edit.remove_media_root(removed.id()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let (later, chunks) = export(&source);
    let later_mirror = SqliteProduction::import_checkpoint(
        directory.path().join("later-roots.pproj"),
        &later,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    let mut reader = source.record_reader(2).unwrap();
    let record = reader.manifest().clone();
    let mut chunks = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        chunks.push(chunk);
    }
    mirror
        .apply_record(&record, chunks.into_iter().map(Ok), ReplayLimits::default())
        .unwrap();
    for store in [&mirror, &later_mirror] {
        assert_eq!(store.exchange_floor().unwrap(), floor);
        assert_eq!(
            store.exchange_head().unwrap(),
            source.exchange_head().unwrap()
        );
        assert_eq!(store.media_roots().unwrap(), source.media_roots().unwrap());
        assert_eq!(
            store.changes_since(0, 10).unwrap(),
            source.changes_since(0, 10).unwrap()
        );
    }
}
