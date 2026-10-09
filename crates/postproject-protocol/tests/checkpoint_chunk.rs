//! Checkpoint bytes bind their export and section rather than borrowing revisions.

use postproject_core::ProductionId;
use postproject_protocol::{
    CheckpointChunk, CheckpointId, CheckpointSection, Document, Extensions, HistoryId, Limits,
    Scope,
};

#[test]
fn section_identity_original_bytes_and_extensions_round_trip() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let id = CheckpointId::new();
    let extensions = Extensions::new(
        Document::parse(
            r#"{"test:exact":["名",null]}"#.as_bytes(),
            Limits::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let chunk = CheckpointChunk::new(
        scope,
        id,
        CheckpointSection::Metadata,
        0,
        None,
        vec![0, 255, 128],
        extensions.clone(),
    )
    .unwrap();
    assert_eq!(chunk.scope(), scope);
    assert_eq!(chunk.checkpoint(), id);
    assert_eq!(chunk.section(), CheckpointSection::Metadata);
    assert_eq!(chunk.index(), 0);
    assert_eq!(chunk.previous(), None);
    assert_eq!(chunk.payload(), [0, 255, 128]);
    assert_eq!(chunk.extensions(), &extensions);
    let bytes = chunk.document().unwrap().canonical_bytes().unwrap();
    assert_eq!(
        CheckpointChunk::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
            .unwrap(),
        chunk
    );
    assert_ne!(
        chunk.digest().unwrap(),
        CheckpointChunk::new(
            scope,
            id,
            CheckpointSection::Production,
            0,
            None,
            chunk.payload().to_vec(),
            extensions
        )
        .unwrap()
        .digest()
        .unwrap()
    );
}

#[test]
fn wrong_features_sections_base64_and_altered_owner_reject() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let id = CheckpointId::new();
    assert!(
        CheckpointChunk::new(
            scope,
            id,
            CheckpointSection::Metadata,
            0,
            None,
            vec![],
            Extensions::default()
        )
        .is_err()
    );
    assert!(
        CheckpointChunk::new(
            scope,
            id,
            CheckpointSection::Metadata,
            1,
            None,
            vec![1],
            Extensions::default()
        )
        .is_err()
    );
    let chunk = CheckpointChunk::new(
        scope,
        id,
        CheckpointSection::Metadata,
        0,
        None,
        vec![1],
        Extensions::default(),
    )
    .unwrap();
    let fields: serde_json::Value =
        serde_json::from_slice(&chunk.document().unwrap().canonical_bytes().unwrap()).unwrap();
    for (field, value) in [
        ("payload", "AQ"),
        ("section", "unknown"),
        ("checkpoint", "00000000-0000-0000-0000-000000000000"),
        ("version", "2"),
    ] {
        let mut fields = fields.clone();
        fields[field] = value.into();
        assert!(
            CheckpointChunk::from_document(
                &Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap()
            )
            .is_err()
        );
    }
}
