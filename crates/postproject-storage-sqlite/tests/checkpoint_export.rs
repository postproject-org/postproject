//! Coherent metadata checkpoint transport, distinct from complete domain support.

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};
use postproject_protocol::{
    CheckpointSection, ConflictVersion, Document, FrameDecoder, Limits, ProductionHeader,
    SnapshotAssertion, decode_conflict_floor, decode_event, decode_revision_observation,
};
use postproject_storage_sqlite::SqliteProduction;

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:unknown:checkpoint").unwrap(),
        PropertyId::new("Exact").unwrap(),
    )
}

#[test]
fn checkpoint_pins_all_sections_and_preserves_original_history_and_versions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut source = SqliteProduction::create(&path, Some("Exact 名".into())).unwrap();
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::u64(u64::MAX))
        .unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::u64(u64::MAX))
        .unwrap();
    let revision = edit.commit().unwrap().revision().unwrap().clone();
    drop(edit);
    let head = source.exchange_head().unwrap();
    let mut concurrent = SqliteProduction::open(&path).unwrap();
    let base = concurrent.read_session().unwrap().decision_base();
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            if chunks.is_empty() {
                let mut edit = concurrent.begin_edit(base)?;
                edit.replace_metadata_values(target, &property(), &[MetadataValue::i64(-1)])?;
                edit.commit()?;
            }
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    assert_eq!(manifest.head(), head);
    assert_eq!(source.exchange_head().unwrap().sequence(), 2);
    assert_ne!(manifest.head(), source.exchange_head().unwrap());
    assert_eq!(manifest.sections().len(), 18);
    for summary in manifest.sections() {
        let selected: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.section() == summary.section())
            .collect();
        if let Some(mut chain) = manifest.section_chain(summary.section()) {
            let documents = decode_section(selected, &mut chain);
            manifest.verify_section(summary.section(), chain).unwrap();
            match summary.section() {
                CheckpointSection::Production => assert_eq!(
                    ProductionHeader::from_document(&documents[0]).unwrap(),
                    ProductionHeader::from_production(source.production())
                ),
                CheckpointSection::Metadata => {
                    let values: Vec<_> = documents
                        .iter()
                        .map(|doc| SnapshotAssertion::from_document(doc).unwrap())
                        .collect();
                    assert_eq!(values.len(), 2);
                    for (position, value) in values.iter().enumerate() {
                        assert_eq!(value.position(), position as u64);
                        assert_eq!(value.value(), &MetadataValue::u64(u64::MAX));
                    }
                }
                CheckpointSection::Revisions => assert_eq!(
                    decode_revision_observation(&documents[0]).unwrap(),
                    revision
                ),
                CheckpointSection::Events => {
                    assert_eq!(
                        decode_event(&documents[0]).unwrap().revision_id(),
                        revision.id()
                    );
                    assert_eq!(decode_event(&documents[1]).unwrap().position(), 1);
                }
                CheckpointSection::ConflictVersions => {
                    let version = ConflictVersion::from_document(&documents[0]).unwrap();
                    assert_eq!(version.revision(), revision.id());
                    assert_eq!(version.sequence(), 1);
                }
                CheckpointSection::ConflictFloor => {
                    assert_eq!(decode_conflict_floor(&documents[0]).unwrap().sequence(), 0);
                }
                CheckpointSection::Records => assert_eq!(summary.items(), 1),
                _ => panic!("unexpected nonempty section"),
            }
        } else {
            assert_eq!(selected.len(), 0);
            assert_eq!(summary.items(), 0);
        }
    }
    let second = source.export_checkpoint(|_| Ok(())).unwrap();
    let third = source.export_checkpoint(|_| Ok(())).unwrap();
    assert_eq!(second.head(), third.head());
    assert_ne!(second.id(), third.id());
    assert_eq!(
        Document::parse(
            &manifest.document().unwrap().canonical_bytes().unwrap(),
            Limits::default()
        )
        .unwrap(),
        manifest.document().unwrap()
    );
}

fn decode_section(
    chunks: Vec<&postproject_protocol::CheckpointChunk>,
    chain: &mut postproject_protocol::CheckpointChunkChain,
) -> Vec<Document> {
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut documents = Vec::new();
    for chunk in chunks {
        chain.push(chunk).unwrap();
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (count, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += count;
            documents.extend(document);
        }
    }
    decoder.finish().unwrap();
    documents
}

#[test]
fn genesis_is_explicit_and_sink_failure_never_returns_a_manifest() {
    let directory = tempfile::tempdir().unwrap();
    let source = SqliteProduction::create(directory.path().join("authority.pproj"), None).unwrap();
    let manifest = source.export_checkpoint(|_| Ok(())).unwrap();
    assert_eq!(manifest.head(), manifest.floor());
    assert_eq!(manifest.head().sequence(), 0);
    assert_eq!(
        manifest
            .sections()
            .iter()
            .filter(|summary| summary.items() != 0)
            .count(),
        2
    );
    let failure = source.export_checkpoint(|_| {
        Err(
            postproject_core::Error::new(postproject_core::ErrorKind::Cancelled, "cancelled")
                .into(),
        )
    });
    assert!(failure.is_err());
    // A failed export releases its view and never changes the source anchor.
    assert_eq!(source.exchange_head().unwrap(), manifest.head());
    assert_eq!(
        source.export_checkpoint(|_| Ok(())).unwrap().head(),
        manifest.head()
    );
}
