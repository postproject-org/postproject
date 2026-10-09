//! Stable complete intent, normalized equality and required-feature rejection.

use postproject_core::{
    DecisionBase, MetadataProperty, MetadataValue, ObjectRef, ProductionId, PropertyId,
    RevisionContext, VocabularyId,
};
use postproject_protocol::{
    ClientId, Command, Document, Extensions, FailureKind, HistoryId, Limits, Proposal,
    ProtocolBase, RequestId, Scope,
};

fn proposal(commands: Vec<Command>) -> Proposal {
    Proposal::new(
        Scope::new(
            ProductionId::from_bytes([1; 16]),
            HistoryId::from_bytes([2; 16]),
        ),
        ClientId::from_bytes([3; 16]),
        RequestId::from_bytes([4; 16]),
        None,
        RevisionContext::default(),
        commands,
        Extensions::default(),
    )
    .unwrap()
}

fn command() -> Command {
    Command::AppendMetadata {
        target: ObjectRef::Production(ProductionId::from_bytes([1; 16])),
        property: MetadataProperty::new(
            VocabularyId::new("urn:unrecognized:exact").unwrap(),
            PropertyId::new("repeated").unwrap(),
        ),
        value: MetadataValue::u64(u64::MAX),
    }
}

#[test]
fn node_budget_rejects_the_same_legal_domain_collection_in_wire_and_typed_proposals() {
    let original = proposal(vec![]);
    let command = Command::ReplaceMetadata {
        target: ObjectRef::Production(original.scope().production()),
        property: MetadataProperty::new(
            VocabularyId::new("urn:test").unwrap(),
            PropertyId::new("many").unwrap(),
        ),
        values: vec![MetadataValue::boolean(true); 340_000],
    };
    let bytes = command.document().unwrap().canonical_bytes().unwrap();
    assert!(bytes.len() < Limits::default().max_bytes());
    assert_eq!(
        Document::parse(&bytes, Limits::default())
            .unwrap_err()
            .kind(),
        FailureKind::LimitExceeded
    );
    assert_eq!(
        Proposal::new(
            original.scope(),
            original.client(),
            original.request(),
            None,
            RevisionContext::default(),
            vec![command],
            Extensions::default()
        )
        .unwrap_err()
        .kind(),
        FailureKind::LimitExceeded
    );
}

#[test]
fn empty_and_ordered_proposals_retain_original_normalized_identity() {
    for value in [proposal(vec![]), proposal(vec![command(), command()])] {
        let encoded = value.document().unwrap().canonical_bytes().unwrap();
        let decoded =
            Proposal::from_document(&Document::parse(&encoded, Limits::default()).unwrap())
                .unwrap();
        assert_eq!(decoded, value);
        assert_eq!(decoded.digest().unwrap(), value.digest().unwrap());
    }
    assert_ne!(
        proposal(vec![command()]).digest().unwrap(),
        proposal(vec![command(), command()]).digest().unwrap()
    );
}

#[test]
fn extensions_context_and_base_are_part_of_request_equality() {
    let original = proposal(vec![]);
    let scope = original.scope();
    let mut digests = vec![original.digest().unwrap()];
    for (base, context, extensions) in [
        (
            Some(
                ProtocolBase::new(
                    scope,
                    DecisionBase::new(scope.production(), None, 0).unwrap(),
                )
                .unwrap(),
            ),
            RevisionContext::default(),
            Extensions::default(),
        ),
        (
            None,
            RevisionContext::new(None, Some("changed intent".into())).unwrap(),
            Extensions::default(),
        ),
        (
            None,
            RevisionContext::default(),
            Extensions::new(
                Document::parse(br#"{"urn:unknown:preserved":["a","a"]}"#, Limits::default())
                    .unwrap(),
            )
            .unwrap(),
        ),
    ] {
        let value = Proposal::new(
            scope,
            original.client(),
            original.request(),
            base,
            context,
            vec![],
            extensions,
        )
        .unwrap();
        let digest = value.digest().unwrap();
        assert!(!digests.contains(&digest));
        digests.push(digest);
    }
}

#[test]
fn scope_command_limits_and_unknown_required_fields_reject() {
    let original = proposal(vec![]);
    let other = Scope::new(original.scope().production(), HistoryId::new());
    let base = ProtocolBase::new(
        other,
        DecisionBase::new(other.production(), None, 0).unwrap(),
    )
    .unwrap();
    assert_eq!(
        Proposal::new(
            original.scope(),
            original.client(),
            original.request(),
            Some(base),
            RevisionContext::default(),
            vec![],
            Extensions::default()
        )
        .unwrap_err()
        .kind(),
        FailureKind::ScopeMismatch
    );
    assert_eq!(
        Proposal::new(
            original.scope(),
            original.client(),
            original.request(),
            None,
            RevisionContext::default(),
            vec![command(); 1001],
            Extensions::default()
        )
        .unwrap_err()
        .kind(),
        FailureKind::LimitExceeded
    );
    let bytes = String::from_utf8(original.document().unwrap().canonical_bytes().unwrap()).unwrap();
    for input in [
        bytes.replace("metadata.v1", "future.required"),
        bytes.replace("\"version\":\"1\"", "\"version\":\"2\""),
    ] {
        assert_eq!(
            Proposal::from_document(&Document::parse(input.as_bytes(), Limits::default()).unwrap())
                .unwrap_err()
                .kind(),
            FailureKind::Unsupported
        );
    }
    let unknown_field = bytes.replacen('{', "{\"critical\":true,", 1);
    assert_eq!(
        Proposal::from_document(
            &Document::parse(unknown_field.as_bytes(), Limits::default()).unwrap()
        )
        .unwrap_err()
        .kind(),
        FailureKind::Malformed
    );
}
