//! Identifier transitions preserve opaque spelling and qualifier distinctions.

use postproject_core::{
    AssetId, ExternalIdentifier, IdentifierScheme, ObjectRef, RepresentationId, ResourceId,
    SemanticConflictKey,
};
use postproject_protocol::{Document, IdentifierAttachment, IdentifierChange, Limits};

#[test]
fn additions_and_removals_preserve_every_exact_target_and_qualifier() {
    for target in [
        ObjectRef::Asset(AssetId::new()),
        ObjectRef::Representation(RepresentationId::new()),
        ObjectRef::Resource(ResourceId::new()),
    ] {
        for qualifier in [None, Some("Case名".to_owned())] {
            let identifier = ExternalIdentifier::new(
                IdentifierScheme::new("unknown:Exact").unwrap(),
                "Value/名\"\\?",
                qualifier,
            )
            .unwrap();
            let attachment = IdentifierAttachment::new(target, identifier.clone()).unwrap();
            for change in [
                IdentifierChange::Added(attachment.clone()),
                IdentifierChange::Removed(attachment.clone()),
            ] {
                let bytes = change.document().unwrap().canonical_bytes().unwrap();
                let decoded = IdentifierChange::from_document(
                    &Document::parse(&bytes, Limits::default()).unwrap(),
                )
                .unwrap();
                assert_eq!(decoded, change);
                assert_eq!(
                    decoded.conflict_key(),
                    SemanticConflictKey::ExternalIdentifier {
                        target,
                        identifier: identifier.clone()
                    }
                );
                assert_eq!(decoded.observation(), change.observation());
                let altered =
                    String::from_utf8(bytes)
                        .unwrap()
                        .replacen('{', "{\"unknown\":null,", 1);
                assert!(
                    IdentifierChange::from_document(
                        &Document::parse(altered.as_bytes(), Limits::default()).unwrap()
                    )
                    .is_err()
                );
            }
        }
    }
}
