//! Replacement aggregates stream individual values without losing order.

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, ProductionId, PropertyId, VocabularyId,
};
use postproject_protocol::{
    Document, Limits, MetadataEffect, MetadataEffectStart, MetadataOperation,
};

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:test:unknown").unwrap(),
        PropertyId::new("ordered").unwrap(),
    )
}

#[test]
fn streamed_headers_preserve_positions_empty_replacements_and_removals() {
    let target = ObjectRef::Production(ProductionId::new());
    for (effect, operation, expected) in [
        (
            MetadataEffect::appended(target, property(), 91, MetadataValue::i64(7)).unwrap(),
            MetadataOperation::Appended(91),
            vec![MetadataValue::i64(7)],
        ),
        (
            MetadataEffect::replaced(target, property(), vec![]),
            MetadataOperation::Replaced,
            vec![],
        ),
        (
            MetadataEffect::removed(target, property()),
            MetadataOperation::Removed,
            vec![],
        ),
        (
            MetadataEffect::replaced(
                target,
                property(),
                vec![
                    MetadataValue::i64(8),
                    MetadataValue::i64(8),
                    MetadataValue::i64(9),
                ],
            ),
            MetadataOperation::Replaced,
            vec![
                MetadataValue::i64(8),
                MetadataValue::i64(8),
                MetadataValue::i64(9),
            ],
        ),
    ] {
        let mut frames = effect.frames();
        let start = MetadataEffectStart::from_document(&frames.next().unwrap().unwrap()).unwrap();
        assert_eq!(start.target(), target);
        assert_eq!(start.property(), &property());
        assert_eq!(start.operation(), operation);
        assert_eq!(start.value_count(), u64::try_from(expected.len()).unwrap());
        let actual = frames
            .map(|document| MetadataEffectStart::decode_value(&document.unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}

#[test]
fn native_replacement_has_no_proposal_command_cap_and_checks_each_value() {
    let target = ObjectRef::Production(ProductionId::new());
    let effect = MetadataEffect::replaced(target, property(), vec![MetadataValue::i64(1); 1001]);
    let mut frames = effect.frames();
    let header = frames.next().unwrap().unwrap();
    assert_eq!(
        MetadataEffectStart::from_document(&header)
            .unwrap()
            .value_count(),
        1001
    );
    let text = String::from_utf8(header.canonical_bytes().unwrap()).unwrap();
    for changed in [
        text.replace("replaced", "removed"),
        text.replace("replaced", "appended"),
        text.replace("1001", "01"),
        text.replace("\"position\":null", "\"position\":\"1\""),
    ] {
        assert!(
            MetadataEffectStart::from_document(
                &Document::parse(changed.as_bytes(), Limits::default()).unwrap()
            )
            .is_err()
        );
    }
    assert_eq!(frames.count(), 1001);
    let wrong = Document::parse(
        br#"{"kind":"metadata.value","value":{"kind":"u64","value":"-1"}}"#,
        Limits::default(),
    )
    .unwrap();
    assert!(MetadataEffectStart::decode_value(&wrong).is_err());
}
