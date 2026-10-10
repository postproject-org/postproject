//! Detached bases keep production and source-history boundaries.

use postproject_core::{DecisionBase, ProductionId, RevisionId};
use postproject_protocol::{FailureKind, HistoryId, ProtocolBase, Scope};

#[test]
fn base_scope_cannot_lose_its_original_history() {
    let production = ProductionId::new();
    let scope = Scope::new(production, HistoryId::new());
    let decision = DecisionBase::new(production, None, 0).unwrap();
    let base = ProtocolBase::new(scope, decision).unwrap();
    assert_eq!(base.scope(), scope);
    assert_eq!(base.decision(), decision);
    let different_history = Scope::new(production, HistoryId::new());
    assert_ne!(
        base,
        ProtocolBase::new(different_history, decision).unwrap()
    );
    assert_eq!(
        ProtocolBase::new(Scope::new(ProductionId::new(), scope.history()), decision)
            .unwrap_err()
            .kind(),
        FailureKind::ScopeMismatch
    );
}

#[test]
fn protocol_ids_require_canonical_text() {
    let history = HistoryId::from_bytes([255; 16]);
    assert_eq!(history.to_string().parse::<HistoryId>().unwrap(), history);
    assert!(
        history
            .to_string()
            .to_uppercase()
            .parse::<HistoryId>()
            .is_err()
    );
    assert!(
        history
            .to_string()
            .replace('-', "")
            .parse::<HistoryId>()
            .is_err()
    );
}

#[test]
fn detached_decisions_have_one_exact_codec_including_explicit_genesis() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    for (revision, sequence) in [(None, 0), (Some(RevisionId::new()), (1 << 53) + 1)] {
        let base = ProtocolBase::new(
            scope,
            DecisionBase::new(scope.production(), revision, sequence).unwrap(),
        )
        .unwrap();
        assert_eq!(ProtocolBase::from_document(&base.document()).unwrap(), base);
        let document = base.document().canonical_bytes().unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&document).unwrap();
        for bad in [
            serde_json::json!(-1),
            serde_json::json!("01"),
            serde_json::json!("18446744073709551616"),
        ] {
            value["sequence"] = bad;
            let bytes = serde_json::to_vec(&value).unwrap();
            assert!(
                postproject_protocol::Document::parse(
                    &bytes,
                    postproject_protocol::Limits::default()
                )
                .and_then(|document| ProtocolBase::from_document(&document))
                .is_err()
            );
        }
    }
}
