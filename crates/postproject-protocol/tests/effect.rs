//! Complete facts preserve positions and cannot be confused with commands.

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, ProductionId, PropertyId, VocabularyId,
};
use postproject_protocol::{Command, Document, FailureKind, Limits, MetadataEffect};

#[test]
fn effects_retain_exact_authoritative_alternatives() {
    let target = ObjectRef::Production(ProductionId::new());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:unknown").unwrap(),
        PropertyId::new("ordered").unwrap(),
    );
    for effect in [
        MetadataEffect::appended(
            target,
            property.clone(),
            9_007_199_254_740_993,
            MetadataValue::u64(u64::MAX),
        )
        .unwrap(),
        MetadataEffect::replaced(
            target,
            property.clone(),
            vec![MetadataValue::i64(-1), MetadataValue::i64(-1)],
        ),
        MetadataEffect::removed(target, property.clone()),
    ] {
        let document = effect.document().unwrap();
        let bytes = document.canonical_bytes().unwrap();
        let parsed = Document::parse(&bytes, Limits::default()).unwrap();
        assert_eq!(MetadataEffect::from_document(&parsed).unwrap(), effect);
        assert_eq!(
            Command::from_document(&document).unwrap_err().kind(),
            FailureKind::Unsupported
        );
    }
    assert!(
        MetadataEffect::appended(target, property, u64::MAX, MetadataValue::boolean(true)).is_err()
    );
}
