//! Scalar intent contains prepared facts, never authoritative effect boundaries.

use postproject_core::{
    ExternalIdentifier, FileFacts, IdentifierScheme, Locator, LocatorAvailability, LocatorId,
    MediaRoot, MediaRootId, ObjectRef, ProductionId, ResourceId, RevisionContext, Timestamp,
};
use postproject_protocol::{
    ClientId, Command, Document, Extensions, FailureKind, HistoryId, IdentifierAttachment, Limits,
    Proposal, RecordFeature, RequestId, Scope,
};

fn commands() -> Vec<Command> {
    let root = MediaRoot::new(
        MediaRootId::new(),
        "shots",
        Some("镜头".into()),
        Some("file:///unavailable/shots/".into()),
        -7,
        true,
    )
    .unwrap();
    let resource = ResourceId::new();
    let locator = Locator::new(
        LocatorId::new(),
        resource,
        "file:///unavailable/a.mov",
        Some(Timestamp::from_unix_micros(-9_007_199_254_740_993)),
        LocatorAvailability::Offline,
    )
    .unwrap();
    let attachment = IdentifierAttachment::new(
        ObjectRef::Resource(resource),
        ExternalIdentifier::new(
            IdentifierScheme::new("unknown:Exact").unwrap(),
            "  镜头/value  ",
            Some("Exact".into()),
        )
        .unwrap(),
    )
    .unwrap();
    vec![
        Command::AddMediaRoot(root.clone()),
        Command::SetMediaRootEnabled {
            root_id: root.id(),
            enabled: false,
        },
        Command::RemoveMediaRoot(root.id()),
        Command::AddLocator(locator.clone()),
        Command::RetireLocator(locator.id()),
        Command::AddIdentifier(attachment.clone()),
        Command::RemoveIdentifier(attachment),
        Command::RecordResourceFileFacts {
            resource_id: resource,
            facts: FileFacts::new(u64::MAX, Some(Timestamp::from_unix_micros(i64::MIN))),
        },
    ]
}

fn proposal(commands: Vec<Command>) -> Proposal {
    Proposal::new(
        Scope::new(ProductionId::new(), HistoryId::new()),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        commands,
        Extensions::default(),
    )
    .unwrap()
}

#[test]
fn every_scalar_media_intent_preserves_checked_exact_values() {
    for command in commands() {
        assert_eq!(command.required_feature(), RecordFeature::Media);
        let bytes = command.document().unwrap().canonical_bytes().unwrap();
        assert_eq!(
            Command::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            command
        );
        let original = proposal(vec![command]);
        assert_eq!(
            original.required_features().into_iter().collect::<Vec<_>>(),
            [RecordFeature::Media]
        );
        assert_eq!(
            Proposal::from_document(&original.document().unwrap()).unwrap(),
            original
        );
    }
}

#[test]
fn state_effects_authoritative_fields_and_coerced_booleans_are_not_intent() {
    for command in commands() {
        let original: serde_json::Value =
            serde_json::from_slice(&command.document().unwrap().canonical_bytes().unwrap())
                .unwrap();
        for field in ["revision", "previous", "authority_time_micros", "claim_id"] {
            let mut changed = original.clone();
            changed[field] = serde_json::Value::Null;
            let document =
                Document::parse(&serde_json::to_vec(&changed).unwrap(), Limits::default()).unwrap();
            assert_eq!(
                Command::from_document(&document).unwrap_err().kind(),
                FailureKind::Malformed
            );
        }
    }
    for input in [
        r#"{"kind":"root.set-enabled","id":"00000001-0001-0001-0001-000000000001","enabled":"false"}"#,
        r#"{"kind":"root.added","root":null}"#,
        r#"{"kind":"locator.retire","id":"invalid"}"#,
    ] {
        assert!(
            Command::from_document(&Document::parse(input.as_bytes(), Limits::default()).unwrap())
                .is_err()
        );
    }
}

#[test]
fn proposal_feature_normalization_never_drops_or_invents_a_requirement() {
    let mut intents = commands();
    intents.push(Command::RemoveMetadata {
        target: ObjectRef::Resource(ResourceId::new()),
        property: postproject_core::MetadataProperty::new(
            postproject_core::VocabularyId::new("urn:Exact").unwrap(),
            postproject_core::PropertyId::new("Exact").unwrap(),
        ),
    });
    let original = proposal(intents);
    assert_eq!(
        original.required_features().into_iter().collect::<Vec<_>>(),
        [RecordFeature::Media, RecordFeature::Metadata]
    );
    let value: serde_json::Value =
        serde_json::from_slice(&original.document().unwrap().canonical_bytes().unwrap()).unwrap();
    for features in [
        serde_json::json!(["metadata.v1"]),
        serde_json::json!(["media.v1"]),
        serde_json::json!(["metadata.v1", "media.v1"]),
        serde_json::json!(["media.v1", "media.v1", "metadata.v1"]),
        serde_json::json!(["future.v1", "media.v1", "metadata.v1"]),
    ] {
        let mut changed = value.clone();
        changed["required_features"] = features;
        let document =
            Document::parse(&serde_json::to_vec(&changed).unwrap(), Limits::default()).unwrap();
        assert!(Proposal::from_document(&document).is_err());
    }
    assert_eq!(
        Proposal::from_document(&original.document().unwrap())
            .unwrap()
            .digest()
            .unwrap(),
        original.digest().unwrap()
    );
}
