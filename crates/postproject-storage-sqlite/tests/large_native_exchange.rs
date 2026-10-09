//! A native replacement may exceed a proposal's total message limit.

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};
use postproject_protocol::Limits;
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};
use rusqlite::Connection;

#[test]
fn one_large_replacement_commits_and_streams_to_a_distinct_mirror() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let mirror_path = directory.path().join("mirror.pproj");
    let mut source = SqliteProduction::create(&source_path, None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &mirror_path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let target = ObjectRef::Production(source.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:exchange:large-native").unwrap(),
        PropertyId::new("values").unwrap(),
    );
    let value = MetadataValue::string("x".repeat(1024 * 1024)).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.replace_metadata_values(target, &property, &vec![value; 65])
        .unwrap();
    let receipt = edit.commit().unwrap();
    assert_eq!(receipt.revision().unwrap().sequence(), 1);
    drop(edit);

    let source_sql = Connection::open(&source_path).unwrap();
    let (maximum, total): (i64, i64) = source_sql
        .query_row(
            "SELECT MAX(length(payload)), SUM(length(payload))
             FROM exchange_effect_fragments",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(maximum <= 1024 * 1024);
    assert!(total > i64::try_from(Limits::default().max_bytes()).unwrap());

    let mut reader = source.record_reader(1).unwrap();
    let manifest = reader.manifest().clone();
    assert_eq!(manifest.effect_count(), 1);
    assert_eq!(manifest.event_count(), 1);
    assert!(manifest.chunks().payload_bytes() > 64 * 1024 * 1024);
    assert!(
        mirror
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default(),
            )
            .unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );

    // Compare stored values one at a time, without materializing the collection
    // or depending on either database's local metadata row IDs.
    let mirror_sql = Connection::open(&mirror_path).unwrap();
    let sql = "SELECT position, encoded_value FROM metadata_assertions ORDER BY position";
    let mut expected = source_sql.prepare(sql).unwrap();
    let mut actual = mirror_sql.prepare(sql).unwrap();
    let mut expected = expected.query([]).unwrap();
    let mut actual = actual.query([]).unwrap();
    let mut count = 0;
    while let Some(row) = expected.next().unwrap() {
        let counterpart = actual.next().unwrap().unwrap();
        assert_eq!(
            row.get::<_, i64>(0).unwrap(),
            counterpart.get::<_, i64>(0).unwrap()
        );
        assert_eq!(
            row.get::<_, Vec<u8>>(1).unwrap(),
            counterpart.get::<_, Vec<u8>>(1).unwrap()
        );
        count += 1;
    }
    assert_eq!(count, 65);
    assert!(actual.next().unwrap().is_none());
}
