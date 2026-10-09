//! Section chaining rejects mixed owners and cannot mistake a prefix for a body.

use postproject_core::ProductionId;
use postproject_protocol::{
    CheckpointChunk, CheckpointChunkChain, CheckpointId, CheckpointSection, Extensions, HistoryId,
    Scope,
};

#[test]
fn ordered_sections_have_exact_totals_and_failures_do_not_advance_the_chain() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let id = CheckpointId::new();
    let section = CheckpointSection::Metadata;
    let first = CheckpointChunk::new(
        scope,
        id,
        section,
        0,
        None,
        vec![1; 10],
        Extensions::default(),
    )
    .unwrap();
    let last = CheckpointChunk::new(
        scope,
        id,
        section,
        1,
        Some(first.digest().unwrap()),
        vec![2; 20],
        Extensions::default(),
    )
    .unwrap();
    let mut chain = CheckpointChunkChain::new(scope, id, section);
    assert!(chain.push(&last).is_err());
    chain.push(&first).unwrap();
    let wrong = CheckpointChunk::new(
        scope,
        id,
        CheckpointSection::Production,
        1,
        Some(first.digest().unwrap()),
        vec![2; 20],
        Extensions::default(),
    )
    .unwrap();
    assert!(chain.push(&wrong).is_err());
    chain.push(&last).unwrap();
    let summary = chain.finish().unwrap();
    assert_eq!(summary.count(), 2);
    assert_eq!(summary.payload_bytes(), 30);
    assert_eq!(summary.last_digest(), last.digest().unwrap());
    assert!(
        CheckpointChunkChain::new(scope, id, section)
            .finish()
            .is_err()
    );
}
