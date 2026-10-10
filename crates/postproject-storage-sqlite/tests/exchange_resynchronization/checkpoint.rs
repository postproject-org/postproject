use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};
use postproject_protocol::{
    ArchiveEvidence, ArchiveFamily, CheckpointSection, FrameDecoder, Limits,
};
use postproject_storage_sqlite::{
    CheckpointLimits, ReplayLimits, ResynchronizationLimits, SqliteProduction,
};

fn portable_evidence(connection: &rusqlite::Connection) -> Vec<Vec<u8>> {
    let mut facts = Vec::new();
    for sql in [
        "SELECT payload FROM exchange_effect_fragments WHERE revision_id IN (SELECT id FROM revisions WHERE sequence <= 2) ORDER BY revision_id, effect_position, fragment_position",
        "SELECT manifest FROM exchange_records WHERE sequence <= 2 ORDER BY sequence",
        "SELECT document FROM exchange_record_chunks WHERE revision_id IN (SELECT id FROM revisions WHERE sequence <= 2) ORDER BY revision_id, position, fragment_position",
        "SELECT anchor_digest FROM exchange_prior_anchors ORDER BY floor_sequence",
    ] {
        facts.extend(
            connection
                .prepare(sql)
                .unwrap()
                .query_map([], |row| row.get::<_, Vec<u8>>(0))
                .unwrap()
                .map(Result::unwrap),
        );
    }
    facts
}

#[test]
fn archive_checkpoint_preserves_large_exact_earlier_evidence_and_new_suffix_without_private_outcomes()
 {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&source_path, None).unwrap();
    let request = super::append(&mut source, 1);
    let target = ObjectRef::Production(source.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:Exact").unwrap(),
        PropertyId::new("Bytes名").unwrap(),
    );
    let value = MetadataValue::bytes(vec![42; 2_097_153]).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &value).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let source_connection = rusqlite::Connection::open(&source_path).unwrap();
    source_connection
        .execute("DELETE FROM exchange_records WHERE sequence = 1", [])
        .unwrap();
    let head = super::base(&source);
    let floor = source
        .resynchronize_exchange_history(head, ResynchronizationLimits::default())
        .unwrap();
    let earlier = portable_evidence(&source_connection);
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    assert_eq!(manifest.floor(), floor);
    assert_eq!(manifest.head(), floor);
    verify_archive_fragments(&chunks);
    let mirror_path = directory.path().join("mirror.pproj");
    let mut mirror = SqliteProduction::import_checkpoint(
        &mirror_path,
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    let mirror_connection = rusqlite::Connection::open(&mirror_path).unwrap();
    assert_eq!(portable_evidence(&mirror_connection), earlier);
    assert_eq!(mirror.metadata_values(target, &property).unwrap(), [value]);
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    assert!(mirror.record_reader(2).is_err());
    assert_eq!(
        mirror
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap(),
        None
    );
    assert_eq!(
        mirror_connection
            .query_row("SELECT COUNT(*) FROM exchange_outcomes", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    super::append(&mut source, 3);
    let mut reader = source.record_reader(3).unwrap();
    mirror
        .apply_record(
            &reader.manifest().clone(),
            std::iter::from_fn(|| reader.next_chunk().transpose()),
            ReplayLimits::default(),
        )
        .unwrap();
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert_eq!(portable_evidence(&mirror_connection), earlier);
    drop(mirror);
    let mirror = SqliteProduction::open(&mirror_path).unwrap();
    assert_eq!(mirror.exchange_floor().unwrap(), floor);
    let copied_path = directory.path().join("copied.pproj");
    let copied = reexport(&mirror, &copied_path);
    assert_eq!(
        copied.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert_eq!(
        portable_evidence(&rusqlite::Connection::open(copied_path).unwrap()),
        earlier
    );
}

fn verify_archive_fragments(chunks: &[postproject_protocol::CheckpointChunk]) {
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut evidence = Vec::new();
    for chunk in chunks
        .iter()
        .filter(|chunk| chunk.section() == CheckpointSection::Archives)
    {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            if let Some(document) = document {
                evidence.push(ArchiveEvidence::from_document(&document).unwrap());
            }
        }
    }
    decoder.finish().unwrap();
    assert!(
        evidence
            .iter()
            .any(|item| item.family() == ArchiveFamily::Anchor)
    );
    assert!(
        evidence
            .iter()
            .any(|item| item.family() == ArchiveFamily::Chunk && item.fragment() > 0)
    );
    assert!(
        evidence
            .iter()
            .any(|item| item.family() == ArchiveFamily::PartialEffect && item.fragment() > 0)
    );
}

fn reexport(source: &SqliteProduction, path: &std::path::Path) -> SqliteProduction {
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    SqliteProduction::import_checkpoint(
        path,
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap()
}
