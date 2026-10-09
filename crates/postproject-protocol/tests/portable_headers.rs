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
