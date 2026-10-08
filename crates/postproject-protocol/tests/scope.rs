//! Detached bases keep production and source-history boundaries.

use postproject_core::{DecisionBase, ProductionId};
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
