//! Individual continuations preserve exact membership and exception facts.

use postproject_core::{
    MAX_CONTENT_MEMBERS, MAX_SEQUENCE_EXCEPTIONS, RepresentationId, ResourceId, ResourceMember,
    ResourceRole,
};
use postproject_protocol::{Document, FailureKind, Limits, SequenceException, StructureMember};

#[test]
fn members_preserve_roles_requiredness_and_domain_positions() {
    for required in [false, true] {
        let item = StructureMember::new(
            RepresentationId::new(),
            MAX_CONTENT_MEMBERS - 1,
            ResourceMember::new(
                ResourceId::new(),
                ResourceRole::new("vendor.example:Unknown-role").unwrap(),
                required,
            ),
        )
        .unwrap();
        let bytes = item.document().canonical_bytes().unwrap();
        let restored =
            StructureMember::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap();
        assert_eq!(restored, item);
        let fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(fields["required"].as_bool(), Some(required));
        assert_eq!(fields["position"].as_str(), Some("99999"));
    }
}

#[test]
fn sparse_exceptions_preserve_signed_extremes_without_expanding_a_sequence() {
    for frame in [i64::MIN, -1, 0, i64::MAX] {
        let item =
            SequenceException::new(RepresentationId::new(), MAX_SEQUENCE_EXCEPTIONS - 1, frame)
                .unwrap();
        let restored = SequenceException::from_document(
            &Document::parse(
                &item.document().canonical_bytes().unwrap(),
                Limits::default(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(restored, item);
        assert_eq!(restored.frame(), frame);
    }
    assert_eq!(
        SequenceException::new(RepresentationId::new(), MAX_SEQUENCE_EXCEPTIONS, 0)
            .unwrap_err()
            .kind(),
        FailureKind::LimitExceeded
    );
}

#[test]
fn member_frames_reject_bad_roles_positions_and_integer_requiredness() {
    let item = StructureMember::new(
        RepresentationId::new(),
        0,
        ResourceMember::new(
            ResourceId::new(),
            ResourceRole::new("test:essence").unwrap(),
            true,
        ),
    )
    .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&item.document().canonical_bytes().unwrap()).unwrap();
    for (key, invalid) in [
        ("role", serde_json::json!("not-namespaced")),
        ("required", serde_json::json!("1")),
        ("position", serde_json::json!("-1")),
        ("position", serde_json::json!("100000")),
        ("position", serde_json::json!("00")),
        ("kind", serde_json::json!("unknown")),
    ] {
        let mut fields = value.clone();
        fields[key] = invalid;
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| StructureMember::from_document(&document))
                .is_err()
        );
    }
    let mut fields = value;
    fields["row_id"] = "1".into();
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| StructureMember::from_document(&document))
            .is_err()
    );
}
