//! Scalar ownership has no implicit resource or fingerprint collection.

use postproject_core::{
    AssetId, ContentStructure, Representation, RepresentationId, RepresentationKind, ResourceId,
};
use postproject_protocol::{Document, FailureKind, Limits, RepresentationHeader};

#[test]
fn all_roles_preserve_original_identity_and_asset_ownership() {
    for kind in [
        RepresentationKind::Original,
        RepresentationKind::Proxy,
        RepresentationKind::Optimized,
        RepresentationKind::Derived,
    ] {
        let representation = Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            kind,
            ContentStructure::single_resource(ResourceId::new()),
            Vec::new(),
        );
        let header =
            RepresentationHeader::new(representation.id(), representation.asset_id(), kind)
                .unwrap();
        assert_eq!(
            header,
            RepresentationHeader::from_representation(&representation)
        );
        let bytes = header.document().unwrap().canonical_bytes().unwrap();
        let restored = RepresentationHeader::from_document(
            &Document::parse(&bytes, Limits::default()).unwrap(),
        )
        .unwrap();
        assert_eq!(restored.id(), representation.id());
        assert_eq!(restored.asset_id(), representation.asset_id());
        assert_eq!(restored.kind(), kind);
        let fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(fields.get("content_structure").is_none());
        assert!(fields.get("fingerprints").is_none());
    }
}

#[test]
fn headers_reject_unsupported_roles_and_unchecked_ownership_fields() {
    let representation = Representation::new(
        RepresentationId::new(),
        AssetId::new(),
        RepresentationKind::Original,
        ContentStructure::single_resource(ResourceId::new()),
        Vec::new(),
    );
    let mut value: serde_json::Value = serde_json::from_slice(
        &RepresentationHeader::from_representation(&representation)
            .document()
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    value["role"] = "unknown".into();
    let error = RepresentationHeader::from_document(
        &Document::parse(&serde_json::to_vec(&value).unwrap(), Limits::default()).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Unsupported);
    value["role"] = "original".into();
    for key in ["structure_kind", "resources", "row_id"] {
        value[key] = "0".into();
        assert!(
            Document::parse(&serde_json::to_vec(&value).unwrap(), Limits::default())
                .and_then(|document| RepresentationHeader::from_document(&document))
                .is_err()
        );
        value.as_object_mut().unwrap().remove(key);
    }
    value["asset_id"] = representation
        .asset_id()
        .to_string()
        .replace('-', "")
        .into();
    assert!(
        Document::parse(&serde_json::to_vec(&value).unwrap(), Limits::default())
            .and_then(|document| RepresentationHeader::from_document(&document))
            .is_err()
    );
}
