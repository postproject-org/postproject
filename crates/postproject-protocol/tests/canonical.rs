//! Fixed canonical bytes and independently published BLAKE3 checks.

use postproject_protocol::{Digest, DigestDomain, Document, Limits};

#[test]
fn canonical_bytes_match_fixed_independent_vectors() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/exchange/canonical-json.json"
    ))
    .unwrap();
    for vector in vectors.as_array().unwrap() {
        let input = vector["input"].as_str().unwrap();
        let canonical = vector["canonical"].as_str().unwrap().as_bytes();
        let document = Document::parse(input.as_bytes(), Limits::default()).unwrap();
        assert_eq!(document.canonical_bytes().unwrap(), canonical);
        assert_eq!(
            Document::parse(canonical, Limits::default()).unwrap(),
            document
        );
    }
}

#[test]
fn digest_omits_only_its_own_field_and_preserves_extensions() {
    let a = Document::parse(
        br#"{"digest":"first","extensions":{"x:digest":"a","digest":"inner"}}"#,
        Limits::default(),
    )
    .unwrap();
    let b = Document::parse(
        br#"{"extensions":{"digest":"inner","x:digest":"a"},"digest":"second"}"#,
        Limits::default(),
    )
    .unwrap();
    let changed = Document::parse(
        br#"{"digest":"first","extensions":{"x:digest":"a","digest":"changed"}}"#,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        a.digest(DigestDomain::Record).unwrap(),
        b.digest(DigestDomain::Record).unwrap()
    );
    assert_ne!(
        a.digest(DigestDomain::Record).unwrap(),
        changed.digest(DigestDomain::Record).unwrap()
    );
    let domains = [
        DigestDomain::Request,
        DigestDomain::Record,
        DigestDomain::Chunk,
        DigestDomain::Manifest,
        DigestDomain::Anchor,
        DigestDomain::State,
    ];
    for (index, domain) in domains.iter().enumerate() {
        for other in &domains[index + 1..] {
            assert_ne!(a.digest(*domain).unwrap(), a.digest(*other).unwrap());
        }
    }
}

#[test]
fn digest_text_requires_exact_lowercase_hex() {
    let digest = Digest::of_canonical_bytes(DigestDomain::Request, b"{}");
    assert_eq!(digest.to_string().parse::<Digest>().unwrap(), digest);
    for invalid in [
        digest.to_string().to_uppercase(),
        "f".repeat(63),
        "g".repeat(64),
        "é".repeat(32),
    ] {
        assert!(invalid.parse::<Digest>().is_err());
    }
}

#[test]
fn upstream_hash_and_derive_key_vectors_match() {
    // First 32 bytes of upstream test_vectors.json cases 0 and 1. These
    // expected values are published independently of this protocol codec.
    let context = "BLAKE3 2019-12-27 16:29:52 test vectors context";
    for (input, hash, derived) in [
        (
            b"".as_slice(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
            "2cc39783c223154fea8dfb7c1b1660f2ac2dcbd1c1de8277b0b0dd39b7e50d7d",
        ),
        (
            b"\0".as_slice(),
            "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213",
            "b3e2e340a117a499c6cf2398a19ee0d29cca2bb7404c73063382693bf66cb06c",
        ),
    ] {
        assert_eq!(blake3::hash(input).to_hex().as_str(), hash);
        assert_eq!(
            blake3::Hash::from(blake3::derive_key(context, input))
                .to_hex()
                .as_str(),
            derived
        );
    }
}
