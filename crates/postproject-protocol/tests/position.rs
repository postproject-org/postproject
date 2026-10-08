//! Anchors remain independent of checkpoint identities and query sessions.

use postproject_core::{DecisionBase, ProductionId, RevisionId};
use postproject_protocol::{
    Digest, DigestDomain, Document, HistoryId, Limits, Position, ProtocolBase, Scope,
};

#[test]
fn anchors_are_stable_for_the_exact_source_floor_only() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let genesis = ProtocolBase::new(
        scope,
        DecisionBase::new(scope.production(), None, 0).unwrap(),
    )
    .unwrap();
    let a = Position::anchor(genesis).unwrap();
    assert_eq!(a, Position::anchor(genesis).unwrap());
    assert_eq!(a.sequence(), 0);
    assert_eq!(a.revision(), None);
    let revision = RevisionId::new();
    let base = ProtocolBase::new(
        scope,
        DecisionBase::new(scope.production(), Some(revision), 1).unwrap(),
    )
    .unwrap();
    let floor = Position::anchor(base).unwrap();
    assert_ne!(a.digest(), floor.digest());
    assert_eq!(floor.decision_base(), base);
    let other = Scope::new(scope.production(), HistoryId::new());
    let different_history =
        Position::anchor(ProtocolBase::new(other, base.decision()).unwrap()).unwrap();
    assert_ne!(floor.digest(), different_history.digest());
}

#[test]
fn contradictory_revision_fields_cannot_form_a_position() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let digest = Digest::of_canonical_bytes(DigestDomain::Record, b"{}");
    assert!(Position::new(scope, None, 1, digest).is_err());
    assert!(Position::new(scope, Some(RevisionId::new()), 0, digest).is_err());
}

#[test]
fn wire_positions_preserve_exact_boundaries_and_reject_noncanonical_fields() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let digest = Digest::of_canonical_bytes(DigestDomain::Record, b"{}");
    for (revision, sequence) in [(None, 0), (Some(RevisionId::new()), i64::MAX as u64)] {
        let position = Position::new(scope, revision, sequence, digest).unwrap();
        let bytes = position.document().canonical_bytes().unwrap();
        assert_eq!(
            Position::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            position
        );
        let text = String::from_utf8(bytes).unwrap();
        let invalid = text.replace(
            &format!("\"sequence\":\"{sequence}\""),
            "\"sequence\":\"01\"",
        );
        assert!(
            Position::from_document(
                &Document::parse(invalid.as_bytes(), Limits::default()).unwrap()
            )
            .is_err()
        );
        let invalid = text.replace(&digest.to_string(), &digest.to_string().to_uppercase());
        assert!(
            Position::from_document(
                &Document::parse(invalid.as_bytes(), Limits::default()).unwrap()
            )
            .is_err()
        );
    }
    assert!(Position::new(scope, Some(RevisionId::new()), i64::MAX as u64 + 1, digest).is_err());
}
