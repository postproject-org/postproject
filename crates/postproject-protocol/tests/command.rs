//! Intent remains ordered, exact and separate from current-state authorization.

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, ProductionId, PropertyId, VocabularyId,
};
use postproject_protocol::{Command, Document, Extensions, FailureKind, Limits};

#[test]
fn metadata_command_intents_remain_distinct() {
    let target = ObjectRef::Production(ProductionId::new());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:unknown:exact").unwrap(),
        PropertyId::new("repeated").unwrap(),
    );
    let value = MetadataValue::u64(u64::MAX);
    for command in [
        Command::AppendMetadata {
            target,
            property: property.clone(),
            value: value.clone(),
        },
        Command::ReplaceMetadata {
            target,
            property: property.clone(),
            values: vec![value.clone(), value],
        },
        Command::ReplaceMetadata {
            target,
            property: property.clone(),
            values: vec![],
        },
        Command::RemoveMetadata { target, property },
    ] {
        let bytes = command.document().unwrap().canonical_bytes().unwrap();
        let document = Document::parse(&bytes, Limits::default()).unwrap();
        assert_eq!(Command::from_document(&document).unwrap(), command);
    }
    let unknown = Document::parse(br#"{"kind":"future.required"}"#, Limits::default()).unwrap();
    assert_eq!(
        Command::from_document(&unknown).unwrap_err().kind(),
        FailureKind::Unsupported
    );
}

#[test]
fn extensions_preserve_values_but_reject_unnamespaced_fields_and_limits() {
    let document = Document::parse(
        br#"{"urn:vendor:fact":["repeat","repeat","9007199254740993"]}"#,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(
        Extensions::new(document.clone()).unwrap().document(),
        &document
    );
    for invalid in [
        br#"{"ordinary":"x"}"#.as_slice(),
        br#"{"missing:":null}"#,
        br#"{":missing":null}"#,
    ] {
        assert_eq!(
            Extensions::new(Document::parse(invalid, Limits::default()).unwrap())
                .unwrap_err()
                .kind(),
            FailureKind::Malformed
        );
    }
    let oversized = format!(r#"{{"urn:large":"{}"}}"#, "x".repeat(64 * 1024));
    let document = Document::parse(oversized.as_bytes(), Limits::default()).unwrap();
    assert_eq!(
        Extensions::new(document).unwrap_err().kind(),
        FailureKind::LimitExceeded
    );
}
