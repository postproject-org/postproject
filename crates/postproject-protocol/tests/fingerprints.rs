//! Snapshot evidence retains exact bytes and distinguishes unknown historical time.

use postproject_core::FingerprintSnapshot;
use postproject_protocol::{
    Document, Limits, decode_fingerprint_snapshot, encode_fingerprint_snapshot,
};

#[test]
fn fingerprint_snapshots_retain_unknown_domains_bytes_and_exact_observation_sequences() {
    for sequence in [
        None,
        Some(1),
        Some(9_007_199_254_740_993),
        Some(i64::MAX.unsigned_abs()),
    ] {
        let snapshot = FingerprintSnapshot::new(
            "UNKNOWN_domain-1",
            u16::MAX,
            vec![0, 255, 128, 1, 254],
            sequence,
        )
        .unwrap();
        let bytes = encode_fingerprint_snapshot(&snapshot)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert_eq!(
            decode_fingerprint_snapshot(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap(),
            snapshot
        );
    }
    for sequence in [0, u64::MAX] {
        let snapshot = FingerprintSnapshot::new("valid", 1, vec![1], Some(sequence)).unwrap();
        assert!(encode_fingerprint_snapshot(&snapshot).is_err());
    }
}

#[test]
fn fingerprint_snapshots_reject_bad_domains_overflow_and_noncanonical_base64() {
    let source = FingerprintSnapshot::new("valid", 1, vec![255], None).unwrap();
    let source: serde_json::Value = serde_json::from_slice(
        &encode_fingerprint_snapshot(&source)
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    for (key, value) in [
        ("algorithm", ""),
        ("algorithm", "unsupported byte!"),
        ("version", "65536"),
        ("version", "01"),
        ("value", ""),
        ("value", "/w"),
        ("value", "/x=="),
        ("value", "/w==\n"),
        ("value", "_w=="),
        ("observed_revision_sequence", "0"),
        ("observed_revision_sequence", "9223372036854775808"),
    ] {
        let mut fields = source.clone();
        fields["fingerprint"][key] = value.into();
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| decode_fingerprint_snapshot(&document))
                .is_err(),
            "{key}"
        );
    }
    let mut fields = source;
    fields["fingerprint"]["row_id"] = "1".into();
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| decode_fingerprint_snapshot(&document))
            .is_err()
    );
}
