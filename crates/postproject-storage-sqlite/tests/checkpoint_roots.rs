//! Logical root state and removed-root guards survive checkpoints and catch-up.

use postproject_core::{
    MediaRoot, MediaRootId, MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId,
};
use postproject_protocol::{CheckpointSection, ConflictVersion, FrameDecoder, Limits};
use postproject_storage_sqlite::{CheckpointLimits, ReplayLimits, SqliteProduction};

fn root(name: &str, enabled: bool) -> MediaRoot {
    MediaRoot::new(
        MediaRootId::new(),
        name,
        Some("Exact 名".into()),
        None,
        i32::MIN,
        enabled,
    )
    .unwrap()
}

#[test]
fn root_removals_original_history_and_semantic_guards_survive_checkpoint_and_suffix() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let first = root("Essence", true);
    let removed = root("Temporary", false);
    let target = ObjectRef::Production(source.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:exact").unwrap(),
        PropertyId::new("roots").unwrap(),
    );
    let mut edit = source.begin_transaction().unwrap();
    edit.add_media_root(first.clone()).unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::string("mixed").unwrap())
        .unwrap();
    edit.add_media_root(removed.clone()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.set_media_root_enabled(first.id(), false).unwrap();
    edit.remove_media_root(removed.id()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut keys = Vec::new();
    for chunk in chunks
        .iter()
        .filter(|chunk| chunk.section() == CheckpointSection::ConflictVersions)
    {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            if let Some(document) = document {
                keys.push(ConflictVersion::from_document(&document).unwrap());
            }
        }
    }
    decoder.finish().unwrap();
    assert_eq!(keys.len(), 3);
    assert!(keys.iter().any(|key| key.key()
        == &postproject_core::SemanticConflictKey::MediaRoot(removed.id())
        && key.sequence() == 2));
    let mut mirror = SqliteProduction::import_checkpoint(
        directory.path().join("mirror.pproj"),
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(mirror.media_roots().unwrap(), source.media_roots().unwrap());
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    assert_eq!(
        mirror.metadata_values(target, &property).unwrap(),
        source.metadata_values(target, &property).unwrap()
    );
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.set_media_root_enabled(first.id(), true).unwrap();
    edit.add_media_root(removed).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut reader = source.record_reader(3).unwrap();
    let record = reader.manifest().clone();
    let mut chunks = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        chunks.push(chunk);
    }
    mirror
        .apply_record(&record, chunks.into_iter().map(Ok), ReplayLimits::default())
        .unwrap();
    assert_eq!(mirror.media_roots().unwrap(), source.media_roots().unwrap());
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    drop(mirror);
    let connection = rusqlite::Connection::open(directory.path().join("mirror.pproj")).unwrap();
    let temporary_tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name LIKE 'checkpoint_%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(temporary_tables, 0);
}

#[test]
fn root_checkpoint_streaming_exceeds_the_public_complete_collection_limit() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    for index in 0..1_001 {
        edit.add_media_root(root(&format!("root-{index}"), index % 2 == 0))
            .unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    assert!(source.media_roots().is_err());
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let summary = manifest
        .sections()
        .iter()
        .find(|section| section.section() == CheckpointSection::Roots)
        .unwrap();
    assert_eq!(summary.items(), 1_001);
    let mirror = SqliteProduction::import_checkpoint(
        directory.path().join("mirror.pproj"),
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    let page = postproject_core::QueryPageRequest::new(1_000, None).unwrap();
    let source_page = source.media_roots_page(&page).unwrap();
    let mirror_page = mirror.media_roots_page(&page).unwrap();
    assert_eq!(source_page.items(), mirror_page.items());
    assert!(source_page.next_cursor().is_some());
    assert!(mirror_page.next_cursor().is_some());
    let next =
        postproject_core::QueryPageRequest::new(1_000, mirror_page.next_cursor().cloned()).unwrap();
    assert_eq!(mirror.media_roots_page(&next).unwrap().items().len(), 1);
}
