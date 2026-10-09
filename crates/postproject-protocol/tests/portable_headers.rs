//! Portable headers retain exact source facts and reject local bookkeeping.

use postproject_core::{
    MetadataProperty, ObjectRef, Production, ProductionId, PropertyId, RevisionId,
    SemanticConflictKey, Timestamp, VocabularyId,
};
use postproject_protocol::{ConflictVersion, Document, Limits, ProductionHeader};

#[test]
fn production_headers_exclude_schema_and_preserve_time_and_optional_name() {
    for name in [None, Some(String::new()), Some("Exact\n名".into())] {
        let production = Production::new(
            ProductionId::new(),
            24,
            Timestamp::from_unix_micros(i64::MIN),
            name,
        );
        let header = ProductionHeader::from_production(&production);
        assert_eq!(header.id(), production.id());
        assert_eq!(header.created_at(), production.created_at());
        assert_eq!(header.display_name(), production.display_name());
        let bytes = header.document().canonical_bytes().unwrap();
        assert!(!String::from_utf8(bytes.clone()).unwrap().contains("schema"));
        assert_eq!(
            ProductionHeader::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap(),
            header
        );
        let mut fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        fields["role"] = "authority".into();
        assert!(
            ProductionHeader::from_document(
                &Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn semantic_versions_preserve_unknown_properties_and_exact_revision_boundaries() {
    let key = SemanticConflictKey::MetadataProperty {
        target: ObjectRef::Production(ProductionId::new()),
        property: MetadataProperty::new(
            VocabularyId::new("urn:unknown:exact").unwrap(),
            PropertyId::new("Unknown").unwrap(),
        ),
    };
    let revision = RevisionId::new();
    for sequence in [1, 9_007_199_254_740_993, i64::MAX as u64] {
        let version = ConflictVersion::new(key.clone(), revision, sequence).unwrap();
        let bytes = version.document().unwrap().canonical_bytes().unwrap();
        assert_eq!(
            ConflictVersion::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap(),
            version
        );
        assert_eq!(version.key(), &key);
        assert_eq!(version.revision(), revision);
        assert_eq!(version.sequence(), sequence);
    }
    assert!(ConflictVersion::new(key.clone(), revision, 0).is_err());
    assert!(ConflictVersion::new(key, revision, u64::MAX).is_err());
}

#[test]
fn snapshot_assertions_preserve_exact_ordered_values_and_reject_row_ids() {
    use postproject_core::MetadataValue;
    use postproject_protocol::SnapshotAssertion;
    let target = ObjectRef::Production(ProductionId::new());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:unknown:exact").unwrap(),
        PropertyId::new("Repeated").unwrap(),
    );
    let value = MetadataValue::list(vec![
        MetadataValue::u64(u64::MAX),
        MetadataValue::u64(u64::MAX),
    ])
    .unwrap();
    let assertion = SnapshotAssertion::new(
        target,
        property.clone(),
        9_007_199_254_740_993,
        value.clone(),
    )
    .unwrap();
    assert_eq!(assertion.target(), target);
    assert_eq!(assertion.property(), &property);
    assert_eq!(assertion.position(), 9_007_199_254_740_993);
    assert_eq!(assertion.value(), &value);
    let bytes = assertion.document().unwrap().canonical_bytes().unwrap();
    assert_eq!(
        SnapshotAssertion::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
            .unwrap(),
        assertion
    );
    let mut fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    fields["row_id"] = "1".into();
    assert!(
        SnapshotAssertion::from_document(
            &Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap()
        )
        .is_err()
    );
    assert!(SnapshotAssertion::new(target, property, u64::MAX, value).is_err());
}

#[test]
fn checkpoint_history_retains_original_context_and_independent_conflict_floor() {
    use postproject_core::{DecisionBase, OriginIdentity, Revision, TransactionId};
    use postproject_protocol::{
        decode_conflict_floor, decode_revision_observation, encode_conflict_floor,
        encode_revision_observation,
    };
    let production = ProductionId::new();
    let revision = Revision::new(
        RevisionId::new(),
        9_007_199_254_740_993,
        TransactionId::new(),
        Timestamp::from_unix_micros(i64::MIN),
        Some(OriginIdentity::new("Original", Some("1".into()), None).unwrap()),
        Some("Exact\n名".into()),
    )
    .unwrap();
    let document = encode_revision_observation(&revision).unwrap();
    assert_eq!(decode_revision_observation(&document).unwrap(), revision);
    for base in [
        DecisionBase::new(production, None, 0).unwrap(),
        DecisionBase::new(production, Some(revision.id()), revision.sequence()).unwrap(),
    ] {
        assert_eq!(
            decode_conflict_floor(&encode_conflict_floor(base)).unwrap(),
            base
        );
    }
    let mut fields: serde_json::Value =
        serde_json::from_slice(&document.canonical_bytes().unwrap()).unwrap();
    fields["row_id"] = "1".into();
    assert!(
        decode_revision_observation(
            &Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap()
        )
        .is_err()
    );
    fields = serde_json::json!({"kind":"conflict.floor", "production":production.to_string(), "revision":null, "sequence":"1"});
    assert!(
        decode_conflict_floor(
            &Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap()
        )
        .is_err()
    );
}
