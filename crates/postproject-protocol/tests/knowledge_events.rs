//! Identifier, activity and dependency observations preserve typed original facts.

use postproject_core::{
    ActivityId, ActivityKind, ActivityRole, AssetId, ExternalIdentifier, IdentifierScheme,
    ObjectRef, RepresentationId, RevisionEvent, RevisionEventKind, RevisionId,
};
use postproject_protocol::{Document, Limits, decode_event, encode_event};

#[test]
fn observations_retain_exact_unknown_vocabulary_and_nullable_edge_roles() {
    let target = ObjectRef::Asset(AssetId::new());
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("unknown.EXACT").unwrap(),
        "名%2f\n",
        Some("opaque qualifier".into()),
    )
    .unwrap();
    let activity_id = ActivityId::new();
    let representation_id = RepresentationId::new();
    let events = [
        RevisionEventKind::ExternalIdentifierAdded {
            target,
            identifier: identifier.clone(),
        },
        RevisionEventKind::ExternalIdentifierRemoved { target, identifier },
        RevisionEventKind::ActivityCreated {
            activity_id,
            kind: ActivityKind::new("unknown:operation").unwrap(),
        },
        RevisionEventKind::ActivityInputAdded {
            activity_id,
            representation_id,
            role: None,
        },
        RevisionEventKind::ActivityOutputAdded {
            activity_id,
            representation_id,
            role: Some(ActivityRole::new("unknown:output-role").unwrap()),
        },
        RevisionEventKind::DependencySetRecorded { representation_id },
    ];
    for kind in events {
        let event = RevisionEvent::new(RevisionId::new(), 7, kind);
        let bytes = encode_event(&event).unwrap().canonical_bytes().unwrap();
        assert_eq!(
            decode_event(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            event
        );
    }
}

#[test]
fn observations_cannot_bypass_activity_vocabulary_or_attachment_boundaries() {
    let input = RevisionEvent::new(
        RevisionId::new(),
        0,
        RevisionEventKind::ActivityInputAdded {
            activity_id: ActivityId::new(),
            representation_id: RepresentationId::new(),
            role: None,
        },
    );
    let mut value: serde_json::Value =
        serde_json::from_slice(&encode_event(&input).unwrap().canonical_bytes().unwrap()).unwrap();
    value["event"]["role"] = "unqualified".into();
    assert!(
        Document::parse(&serde_json::to_vec(&value).unwrap(), Limits::default())
            .and_then(|document| decode_event(&document))
            .is_err()
    );
    value["event"]["role"] = serde_json::Value::Null;
    value["event"]["snapshot"] = "current".into();
    assert!(
        Document::parse(&serde_json::to_vec(&value).unwrap(), Limits::default())
            .and_then(|document| decode_event(&document))
            .is_err()
    );
}
