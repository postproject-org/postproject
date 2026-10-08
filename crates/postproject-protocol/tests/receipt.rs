//! Exact public receipts retain their own committed identity and no-op result.

use postproject_core::{
    CommitReceipt, OriginIdentity, ProductionId, Revision, RevisionId, Timestamp, TransactionId,
};
use postproject_protocol::{Document, Limits, decode_receipt, encode_receipt};

#[test]
fn exact_receipts_preserve_their_own_revision_and_no_op() {
    let production = ProductionId::new();
    let revision = Revision::new(
        RevisionId::new(),
        i64::MAX as u64,
        TransactionId::new(),
        Timestamp::from_unix_micros(i64::MIN),
        Some(OriginIdentity::new("Host", Some("1".into()), None).unwrap()),
        Some("Exact receipt".into()),
    )
    .unwrap();
    for receipt in [
        CommitReceipt::new(production, None),
        CommitReceipt::new(production, Some(revision)),
    ] {
        let document = encode_receipt(&receipt).unwrap();
        let bytes = document.canonical_bytes().unwrap();
        let document = Document::parse(&bytes, Limits::default()).unwrap();
        assert_eq!(decode_receipt(&document).unwrap(), receipt);
    }
}

#[test]
fn contradictory_and_lossy_revision_fields_reject() {
    let revision = Revision::new(
        RevisionId::new(),
        1,
        TransactionId::new(),
        Timestamp::from_unix_micros(0),
        None,
        None,
    )
    .unwrap();
    let receipt = CommitReceipt::new(ProductionId::new(), Some(revision));
    let bytes =
        String::from_utf8(encode_receipt(&receipt).unwrap().canonical_bytes().unwrap()).unwrap();
    for replacement in ["0", "01", "9223372036854775808"] {
        let invalid = bytes.replace(
            "\"sequence\":\"1\"",
            &format!("\"sequence\":\"{replacement}\""),
        );
        assert!(
            decode_receipt(&Document::parse(invalid.as_bytes(), Limits::default()).unwrap())
                .is_err()
        );
    }
}
