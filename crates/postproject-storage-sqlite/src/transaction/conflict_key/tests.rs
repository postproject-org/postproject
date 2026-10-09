use super::*;
use postproject_core::{ActivityId, AssetId, JobId, ProductionId, RevisionId};
use postproject_protocol::ConflictVersion;

fn keys() -> Vec<SemanticConflictKey> {
    let resource = ResourceId::new();
    let representation = RepresentationId::new();
    let mut keys = vec![
        SemanticConflictKey::LocatorSet(resource),
        SemanticConflictKey::DependencySet(representation),
        SemanticConflictKey::MediaRoot(MediaRootId::new()),
        SemanticConflictKey::ResourceFileFacts(resource),
        SemanticConflictKey::ResourceFingerprint {
            resource_id: resource,
            algorithm: "vendor_Unknown".into(),
            version: u16::MAX,
        },
        SemanticConflictKey::RepresentationFingerprint {
            representation_id: representation,
            algorithm: "vendor_Unknown".into(),
            version: 0,
        },
    ];
    for target in [
        ObjectRef::Production(ProductionId::new()),
        ObjectRef::Asset(AssetId::new()),
        ObjectRef::Representation(representation),
        ObjectRef::Resource(resource),
        ObjectRef::Activity(ActivityId::new()),
        ObjectRef::Job(JobId::new()),
    ] {
        keys.push(SemanticConflictKey::MetadataProperty {
            target,
            property: MetadataProperty::new(
                VocabularyId::new("urn:未知:Exact").unwrap(),
                PropertyId::new("Repeated").unwrap(),
            ),
        });
        for qualifier in [None, Some("Case 😀 / same".to_owned())] {
            keys.push(SemanticConflictKey::ExternalIdentifier {
                target,
                identifier: ExternalIdentifier::new(
                    IdentifierScheme::new("vendor:Unknown").unwrap(),
                    "Exact 😀 / same",
                    qualifier,
                )
                .unwrap(),
            });
        }
    }
    keys
}

#[test]
fn every_index_key_decodes_to_the_same_portable_domain_fact() {
    for key in keys() {
        let bytes = super::super::encode_conflict_key(&key).unwrap();
        assert_eq!(decode(&bytes).unwrap(), key);
        let version = ConflictVersion::new(decode(&bytes).unwrap(), RevisionId::new(), 1).unwrap();
        assert_eq!(
            ConflictVersion::from_document(&version.document().unwrap()).unwrap(),
            version
        );
    }
    let key = SemanticConflictKey::ExternalIdentifier {
        target: ObjectRef::Job(JobId::new()),
        identifier: ExternalIdentifier::new(
            IdentifierScheme::new("x".repeat(MAX_IDENTIFIER_SCHEME_BYTES)).unwrap(),
            "x".repeat(MAX_IDENTIFIER_VALUE_BYTES),
            Some("x".repeat(MAX_IDENTIFIER_QUALIFIER_BYTES)),
        )
        .unwrap(),
    };
    assert_eq!(
        decode(&super::super::encode_conflict_key(&key).unwrap()).unwrap(),
        key
    );
}

#[test]
fn corrupt_keys_fail_without_panics_or_copying_unbounded_strings() {
    for key in keys() {
        let bytes = super::super::encode_conflict_key(&key).unwrap();
        for end in 0..bytes.len() {
            assert_eq!(
                decode(&bytes[..end]).unwrap_err().kind(),
                ErrorKind::Storage
            );
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert_eq!(decode(&extra).unwrap_err().kind(), ErrorKind::Storage);
        let mut tag = bytes;
        tag[0] = 255;
        assert_eq!(decode(&tag).unwrap_err().kind(), ErrorKind::Storage);
    }
    assert_eq!(
        decode(&vec![0; 6000]).unwrap_err().kind(),
        ErrorKind::Storage
    );
    let mut length = vec![6];
    length.extend_from_slice(ResourceId::new().as_bytes());
    length.extend_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(decode(&length).unwrap_err().kind(), ErrorKind::Storage);
    // Unknown qualifier alternatives cannot become an absent qualifier.
    let key = SemanticConflictKey::ExternalIdentifier {
        target: ObjectRef::Asset(AssetId::new()),
        identifier: ExternalIdentifier::new(IdentifierScheme::new("x").unwrap(), "x", None)
            .unwrap(),
    };
    let mut bytes = super::super::encode_conflict_key(&key).unwrap();
    *bytes.last_mut().unwrap() = 2;
    assert_eq!(decode(&bytes).unwrap_err().kind(), ErrorKind::Storage);
}
