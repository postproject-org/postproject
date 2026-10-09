//! Observation identity and ordering survive exchange independently of effects.

use postproject_core::{
    MetadataProperty, ObjectRef, ProductionId, PropertyId, RevisionEvent, RevisionEventKind,
    RevisionId, VocabularyId,
};
use postproject_protocol::{Document, Limits, decode_event, encode_event};

#[test]
fn original_metadata_observations_round_trip_exactly() {
    let target = ObjectRef::Production(ProductionId::new());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:unknown:test").unwrap(),
        PropertyId::new("repeated").unwrap(),
    );
    for kind in [
        RevisionEventKind::MetadataAddedOrReplaced {
            target,
            property: property.clone(),
        },
        RevisionEventKind::MetadataRemoved { target, property },
    ] {
        let event = RevisionEvent::new(RevisionId::new(), u32::MAX, kind);
        let bytes = encode_event(&event).unwrap().canonical_bytes().unwrap();
        assert_eq!(
            decode_event(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            event
        );
        let text = String::from_utf8(bytes).unwrap();
        let changed = text.replace(&u32::MAX.to_string(), "4294967296");
        assert!(
            decode_event(&Document::parse(changed.as_bytes(), Limits::default()).unwrap()).is_err()
        );
    }
    let unknown = format!(
        r#"{{"kind":"observation","revision":"{}","position":"0","event":{{"kind":"future_event"}}}}"#,
        RevisionId::new()
    );
    assert!(
        decode_event(&Document::parse(unknown.as_bytes(), Limits::default()).unwrap()).is_err()
    );
}
