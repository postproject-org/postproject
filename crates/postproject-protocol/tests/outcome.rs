//! Stable public outcomes preserve complete request identity and extensions.

use postproject_core::{CommitReceipt, Error, ErrorKind, ProductionId, RevisionContext};
use postproject_protocol::{
    ClientId, Document, Extensions, HistoryId, Limits, Outcome, Proposal, Rejection, RequestId,
    Scope,
};

#[test]
fn no_op_and_rejection_preserve_identity_and_extensions_without_diagnostics() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let extensions = Extensions::new(
        Document::parse(
            br#"{"vendor:opaque":{"ordered":["x","x"]}}"#,
            Limits::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let proposal = Proposal::new(
        scope,
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![],
        extensions,
    )
    .unwrap();
    let rejection = Rejection::domain(&Error::new(
        ErrorKind::NotFound,
        "private path or credential",
    ))
    .unwrap();
    for outcome in [
        Outcome::accepted(&proposal, CommitReceipt::new(scope.production(), None)).unwrap(),
        Outcome::rejected(&proposal, rejection).unwrap(),
    ] {
        assert_eq!(outcome.request_digest(), proposal.digest().unwrap());
        assert_eq!(outcome.extensions(), proposal.extensions());
        let bytes = outcome.document().unwrap().canonical_bytes().unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("private"));
        assert_eq!(
            Outcome::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            outcome
        );
    }
    assert!(Outcome::accepted(&proposal, CommitReceipt::new(ProductionId::new(), None)).is_err());
}
