//! Unknown identifier vocabularies retain exact spelling without new standards mappings.

use postproject_core::{
    AssetId, ExternalIdentifier, IdentifierScheme, ObjectRef, ProductionId, RepresentationId,
    ResourceId,
};
use postproject_protocol::{Document, FailureKind, IdentifierAttachment, Limits};

fn identifier() -> ExternalIdentifier {
    ExternalIdentifier::new(
        IdentifierScheme::new("UnKnOwN.名:scheme").unwrap(),
        "opaque EXACT%2f 名\n",
        Some("scope:EXACT".into()),
    )
    .unwrap()
}

#[test]
fn attachments_retain_all_supported_targets_and_exact_external_values() {
    for target in [
        ObjectRef::Asset(AssetId::new()),
        ObjectRef::Representation(RepresentationId::new()),
        ObjectRef::Resource(ResourceId::new()),
    ] {
        for qualifier in [None, Some("scope:EXACT".into())] {
            let original = identifier();
            let identifier =
                ExternalIdentifier::new(original.scheme().clone(), original.value(), qualifier)
                    .unwrap();
            let attachment = IdentifierAttachment::new(target, identifier).unwrap();
            let bytes = attachment.document().unwrap().canonical_bytes().unwrap();
            let restored = IdentifierAttachment::from_document(
                &Document::parse(&bytes, Limits::default()).unwrap(),
            )
            .unwrap();
            assert_eq!(restored, attachment);
            assert_eq!(restored.identifier().scheme().as_str(), "UnKnOwN.名:scheme");
        }
    }
    assert_eq!(
        IdentifierAttachment::new(ObjectRef::Production(ProductionId::new()), identifier())
            .unwrap_err()
            .kind(),
        FailureKind::Unsupported
    );
}

#[test]
fn malformed_identifier_payloads_cannot_bypass_native_value_rules() {
    let source = IdentifierAttachment::new(ObjectRef::Asset(AssetId::new()), identifier()).unwrap();
    let source: serde_json::Value =
        serde_json::from_slice(&source.document().unwrap().canonical_bytes().unwrap()).unwrap();
    for (key, value) in [
        ("scheme", "bad scheme".into()),
        ("value", String::new()),
        ("value", "nul\0".into()),
        ("qualifier", String::new()),
        ("qualifier", "x".repeat(1025)),
    ] {
        let mut fields = source.clone();
        fields["identifier"][key] = value.into();
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| IdentifierAttachment::from_document(&document))
                .is_err()
        );
    }
    let mut fields = source;
    fields["row_id"] = "1".into();
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| IdentifierAttachment::from_document(&document))
            .is_err()
    );
}
