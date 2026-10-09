//! Bounded effect pieces preserve the exact older canonical evidence document.

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, ProductionId, PropertyId, VocabularyId,
};
use postproject_protocol::{Limits, MetadataEffect};

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("unknown:名").unwrap(),
        PropertyId::new("Exact_property").unwrap(),
    )
}

#[test]
fn streamed_bytes_match_all_effect_alternatives_empty_sets_and_ordered_values() {
    let target = ObjectRef::Production(ProductionId::new());
    let values = vec![
        MetadataValue::i64(i64::MIN),
        MetadataValue::u64(u64::MAX),
        MetadataValue::string("名\n\\\"").unwrap(),
        MetadataValue::bytes(vec![0, 255, 1]).unwrap(),
    ];
    let effects = [
        MetadataEffect::appended(target, property(), 7, values[2].clone()).unwrap(),
        MetadataEffect::replaced(target, property(), values),
        MetadataEffect::replaced(target, property(), Vec::new()),
        MetadataEffect::removed(target, property()),
    ];
    for effect in effects {
        let expected = effect.document().unwrap().canonical_bytes().unwrap();
        let actual: Vec<_> = effect
            .canonical_parts()
            .unwrap()
            .flat_map(Result::unwrap)
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn a_native_replacement_larger_than_one_message_keeps_individual_parts_bounded() {
    let target = ObjectRef::Production(ProductionId::new());
    let value = MetadataValue::string("x".repeat(1024 * 1024)).unwrap();
    let effect = MetadataEffect::replaced(target, property(), vec![value; 65]);
    let mut total = 0;
    let mut parts = 0;
    for bytes in effect.canonical_parts().unwrap() {
        let bytes = bytes.unwrap();
        assert!(bytes.len() < 2 * 1024 * 1024);
        total += bytes.len();
        parts += 1;
    }
    assert!(total > Limits::default().max_bytes());
    assert_eq!(parts, 67);
}
