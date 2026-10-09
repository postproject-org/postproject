//! Fixed expected bytes/digests authored by Python without the `PostProject` codec.

use std::path::Path;

use postproject_protocol::{Document, Limits, Proposal};

#[test]
fn independently_authored_family_proposals_have_exact_canonical_bytes_and_digests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/exchange");
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/exchange/proposal-vectors.json"
    ))
    .unwrap();
    for vector in vectors.as_array().unwrap() {
        let family = vector["family"].as_str().unwrap();
        let input = std::fs::read(root.join(vector["file"].as_str().unwrap())).unwrap();
        let document = Document::parse(&input, Limits::default()).unwrap();
        let expected = vector["canonical"].as_str().unwrap().as_bytes();
        assert_eq!(document.canonical_bytes().unwrap(), expected, "{family}");
        let proposal = Proposal::from_document(&document).unwrap();
        assert_eq!(
            proposal.document().unwrap().canonical_bytes().unwrap(),
            expected,
            "{family}"
        );
        assert_eq!(
            proposal.digest().unwrap().to_string(),
            vector["request_digest"].as_str().unwrap(),
            "{family}"
        );
    }
}
