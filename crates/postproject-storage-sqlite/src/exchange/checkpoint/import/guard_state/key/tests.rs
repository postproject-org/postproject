use postproject_core::{
    ExternalIdentifier, IdentifierScheme, LocatorId, MediaRootId, MetadataProperty, ObjectRef,
    ProductionId, PropertyId, RepresentationId, ResourceId, RevisionEventKind as Event,
    SemanticConflictKey as Key, VocabularyId,
};

#[test]
fn location_metadata_and_identifier_observations_keep_exact_keys() {
    let resource = ResourceId::new();
    let root = MediaRootId::new();
    let target = ObjectRef::Production(ProductionId::new());
    let identifier_target = ObjectRef::Resource(resource);
    let property = MetadataProperty::new(
        VocabularyId::new("unknown:Exact:名").unwrap(),
        PropertyId::new("Prop").unwrap(),
    );
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("unknown:CASE").unwrap(),
        "Exact\u{301}名",
        Some("Qual 名".into()),
    )
    .unwrap();
    let cases = [
        (
            Event::LocatorAdded {
                resource_id: resource,
                locator_id: LocatorId::new(),
            },
            Key::LocatorSet(resource),
        ),
        (
            Event::LocatorRetired {
                resource_id: resource,
                locator_id: LocatorId::new(),
            },
            Key::LocatorSet(resource),
        ),
        (
            Event::MediaRootAdded {
                media_root_id: root,
            },
            Key::MediaRoot(root),
        ),
        (
            Event::MediaRootEnabledChanged {
                media_root_id: root,
                enabled: false,
            },
            Key::MediaRoot(root),
        ),
        (
            Event::MediaRootRemoved {
                media_root_id: root,
            },
            Key::MediaRoot(root),
        ),
        (
            Event::MetadataAddedOrReplaced {
                target,
                property: property.clone(),
            },
            Key::MetadataProperty {
                target,
                property: property.clone(),
            },
        ),
        (
            Event::MetadataRemoved {
                target,
                property: property.clone(),
            },
            Key::MetadataProperty { target, property },
        ),
        (
            Event::ExternalIdentifierAdded {
                target: identifier_target,
                identifier: identifier.clone(),
            },
            Key::ExternalIdentifier {
                target: identifier_target,
                identifier: identifier.clone(),
            },
        ),
        (
            Event::ExternalIdentifierRemoved {
                target: identifier_target,
                identifier: identifier.clone(),
            },
            Key::ExternalIdentifier {
                target: identifier_target,
                identifier,
            },
        ),
    ];
    for (event, expected) in cases {
        assert_key(&event, &expected);
    }
}

#[test]
fn fingerprint_and_dependency_observations_keep_their_distinct_domains() {
    let resource = ResourceId::new();
    let representation = RepresentationId::new();
    let cases = [
        (
            Event::ResourceFingerprintObserved {
                resource_id: resource,
                algorithm: "UNKNOWN_hash".into(),
                version: u16::MAX,
            },
            Key::ResourceFingerprint {
                resource_id: resource,
                algorithm: "UNKNOWN_hash".into(),
                version: u16::MAX,
            },
        ),
        (
            Event::RepresentationFingerprintObserved {
                representation_id: representation,
                algorithm: "Unknown_Hash".into(),
                version: 0,
            },
            Key::RepresentationFingerprint {
                representation_id: representation,
                algorithm: "Unknown_Hash".into(),
                version: 0,
            },
        ),
        (
            Event::ResourceFileFactsObserved {
                resource_id: resource,
            },
            Key::ResourceFileFacts(resource),
        ),
        (
            Event::DependencySetRecorded {
                representation_id: representation,
            },
            Key::DependencySet(representation),
        ),
    ];
    for (event, expected) in cases {
        assert_key(&event, &expected);
    }
    for event in [
        Event::ResourceAdded {
            resource_id: resource,
        },
        Event::JobClaimed {
            job_id: postproject_core::JobId::new(),
        },
    ] {
        assert!(super::from_event(&event).unwrap().is_none());
    }
}

fn assert_key(event: &Event, expected: &Key) {
    let actual = super::from_event(event).unwrap().unwrap();
    assert_eq!(&actual, expected);
    let encoded = crate::transaction::encode_conflict_key(&actual).unwrap();
    assert_eq!(
        &crate::transaction::decode_conflict_key(&encoded).unwrap(),
        expected
    );
}
