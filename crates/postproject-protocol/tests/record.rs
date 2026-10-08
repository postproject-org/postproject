//! A bounded manifest commits to original history and the entire streamed body.

use postproject_core::{ProductionId, Revision, RevisionId, Timestamp, TransactionId};
use postproject_protocol::{
    Digest, DigestDomain, Extensions, FailureKind, HistoryId, Position, RecordChunk,
    RecordManifest, Scope,
};

fn revision(sequence: u64) -> Revision {
    Revision::new(
        RevisionId::new(),
        sequence,
        TransactionId::new(),
        Timestamp::from_unix_micros(-123),
        None,
        Some("Original context".into()),
    )
    .unwrap()
}

#[test]
fn manifest_checks_the_entire_chain_and_retains_original_revision() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let predecessor = Position::new(scope, None, 0, Digest::from_bytes([7; 32])).unwrap();
    let revision = revision(1);
    let first = RecordChunk::new(
        scope,
        revision.id(),
        0,
        None,
        vec![1],
        Extensions::default(),
    )
    .unwrap();
    let last = RecordChunk::new(
        scope,
        revision.id(),
        1,
        Some(first.digest().unwrap()),
        vec![2],
        Extensions::default(),
    )
    .unwrap();
    let mut chain = postproject_protocol::RecordChunkChain::new(scope, revision.id());
    chain.push(&first).unwrap();
    let partial = chain.clone();
    chain.push(&last).unwrap();
    let manifest = RecordManifest::new(
        predecessor,
        revision.clone(),
        chain.clone().finish().unwrap(),
        1,
        1,
        Extensions::default(),
    )
    .unwrap();
    assert_eq!(
        manifest.verify_chain(partial).unwrap_err().kind(),
        FailureKind::Integrity
    );
    manifest.verify_chain(chain).unwrap();
    assert_eq!(manifest.revision(), &revision);
    assert_eq!(manifest.head().unwrap().revision(), Some(revision.id()));
    assert_eq!(manifest.head().unwrap().scope(), scope);
    assert_eq!(
        manifest.head().unwrap().digest(),
        manifest.record_digest().unwrap()
    );
    assert_ne!(
        manifest
            .document()
            .unwrap()
            .digest(DigestDomain::Manifest)
            .unwrap(),
        manifest.record_digest().unwrap()
    );
    let foreign = postproject_protocol::RecordChunkChain::new(
        Scope::new(scope.production(), HistoryId::new()),
        revision.id(),
    );
    assert_eq!(
        manifest.verify_chain(foreign).unwrap_err().kind(),
        FailureKind::ScopeMismatch
    );
    let changed_context = Revision::new(
        revision.id(),
        1,
        revision.transaction_id(),
        revision.committed_at(),
        None,
        Some("Altered".into()),
    )
    .unwrap();
    let changed = RecordManifest::new(
        predecessor,
        changed_context,
        manifest.chunks(),
        1,
        1,
        Extensions::default(),
    )
    .unwrap();
    assert_ne!(
        changed.record_digest().unwrap(),
        manifest.record_digest().unwrap()
    );
}

#[test]
fn manifest_rejects_skipped_repeated_empty_and_out_of_range_history() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let revision = revision(2);
    let predecessor = Position::new(
        scope,
        Some(RevisionId::new()),
        1,
        Digest::from_bytes([0; 32]),
    )
    .unwrap();
    let chunks =
        postproject_protocol::ChunkSummary::new(1, 1, Digest::from_bytes([1; 32])).unwrap();
    let create = |predecessor, revision, effects, events| {
        RecordManifest::new(
            predecessor,
            revision,
            chunks,
            effects,
            events,
            Extensions::default(),
        )
    };
    assert!(create(predecessor, revision.clone(), 1, 1).is_ok());
    assert!(create(predecessor, super_revision(&revision, 3), 1, 1).is_err());
    let repeated = Position::new(scope, Some(revision.id()), 1, predecessor.digest()).unwrap();
    assert!(create(repeated, revision.clone(), 1, 1).is_err());
    assert!(create(predecessor, revision.clone(), 0, 1).is_err());
    assert!(create(predecessor, revision.clone(), 1, 0).is_err());
    assert!(create(predecessor, revision.clone(), u64::MAX, 1).is_err());
    let high = Position::new(
        scope,
        Some(RevisionId::new()),
        i64::MAX as u64,
        predecessor.digest(),
    )
    .unwrap();
    assert!(create(high, super_revision(&revision, i64::MAX as u64 + 1), 1, 1).is_err());
}

fn super_revision(original: &Revision, sequence: u64) -> Revision {
    Revision::new(
        original.id(),
        sequence,
        original.transaction_id(),
        original.committed_at(),
        original.origin().cloned(),
        original.message().map(str::to_owned),
    )
    .unwrap()
}
