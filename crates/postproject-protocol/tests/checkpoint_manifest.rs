//! Completeness declarations do not replace validation of source facts.

use postproject_core::{DecisionBase, ProductionId};
use postproject_protocol::{
    CheckpointId, CheckpointManifest, CheckpointSection, ChunkSummary, Digest, Document,
    Extensions, HistoryId, Limits, Position, ProtocolBase, Scope, SectionSummary,
};

fn anchor() -> Position {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    Position::anchor(
        ProtocolBase::new(
            scope,
            DecisionBase::new(scope.production(), None, 0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn summaries() -> [SectionSummary; CheckpointSection::ALL.len()] {
    let chunk = ChunkSummary::new(1, 100, Digest::from_bytes([7; 32])).unwrap();
    CheckpointSection::ALL.map(|section| {
        if matches!(
            section,
            CheckpointSection::Production | CheckpointSection::ConflictFloor
        ) {
            SectionSummary::new(section, 1, Some(chunk)).unwrap()
        } else {
            SectionSummary::new(section, 0, None).unwrap()
        }
    })
}

#[test]
fn new_exports_keep_the_same_source_continuation_with_distinct_manifests() {
    let head = anchor();
    let first = CheckpointManifest::new(
        CheckpointId::new(),
        head,
        head,
        summaries(),
        Extensions::default(),
    )
    .unwrap();
    let second = CheckpointManifest::new(
        CheckpointId::new(),
        head,
        head,
        summaries(),
        Extensions::default(),
    )
    .unwrap();
    assert_ne!(first.id(), second.id());
    assert_eq!(first.head(), second.head());
    assert_eq!(first.floor(), head);
    assert_eq!(first.sections(), &summaries());
    assert_eq!(first.extensions(), &Extensions::default());
    assert_ne!(first.document().unwrap(), second.document().unwrap());
    let bytes = first.document().unwrap().canonical_bytes().unwrap();
    assert_eq!(
        CheckpointManifest::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
            .unwrap(),
        first
    );
}

#[test]
fn missing_reordered_foreign_and_altered_declarations_reject() {
    let head = anchor();
    let mut sections = summaries();
    sections.swap(1, 2);
    assert!(
        CheckpointManifest::new(
            CheckpointId::new(),
            head,
            head,
            sections,
            Extensions::default()
        )
        .is_err()
    );
    assert!(
        CheckpointManifest::new(
            CheckpointId::new(),
            head,
            anchor(),
            summaries(),
            Extensions::default()
        )
        .is_err()
    );
    let manifest = CheckpointManifest::new(
        CheckpointId::new(),
        head,
        head,
        summaries(),
        Extensions::default(),
    )
    .unwrap();
    let fields: serde_json::Value =
        serde_json::from_slice(&manifest.document().unwrap().canonical_bytes().unwrap()).unwrap();
    for change in 0..4 {
        let mut fields = fields.clone();
        match change {
            0 => {
                fields["sections"].as_array_mut().unwrap().pop();
            }
            1 => fields["sections"][1]["section"] = "unknown".into(),
            2 => fields["required_features"] = serde_json::json!(["unknown.v1"]),
            _ => fields["checkpoint"] = CheckpointId::new().to_string().into(),
        }
        assert!(
            CheckpointManifest::from_document(
                &Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn manifests_verify_whole_section_commitments_and_reject_prefixes_or_foreign_chains() {
    use postproject_protocol::{CheckpointChunk, CheckpointChunkChain};
    let head = anchor();
    let id = CheckpointId::new();
    let section = CheckpointSection::Production;
    let first = CheckpointChunk::new(
        head.scope(),
        id,
        section,
        0,
        None,
        vec![1; 10],
        Extensions::default(),
    )
    .unwrap();
    let last = CheckpointChunk::new(
        head.scope(),
        id,
        section,
        1,
        Some(first.digest().unwrap()),
        vec![2; 20],
        Extensions::default(),
    )
    .unwrap();
    let mut chain = CheckpointChunkChain::new(head.scope(), id, section);
    chain.push(&first).unwrap();
    let prefix = chain.clone();
    chain.push(&last).unwrap();
    let summary = chain.clone().finish().unwrap();
    let mut sections = summaries();
    sections[0] = SectionSummary::new(section, 1, Some(summary)).unwrap();
    let manifest =
        CheckpointManifest::new(id, head, head, sections, Extensions::default()).unwrap();
    assert!(
        manifest
            .section_chain(CheckpointSection::Metadata)
            .is_none()
    );
    let mut verifier = manifest.section_chain(section).unwrap();
    verifier.push(&first).unwrap();
    verifier.push(&last).unwrap();
    manifest.verify_section(section, verifier).unwrap();
    assert!(manifest.verify_section(section, prefix).is_err());
    let foreign = CheckpointChunkChain::new(head.scope(), CheckpointId::new(), section);
    assert!(manifest.verify_section(section, foreign).is_err());
}
