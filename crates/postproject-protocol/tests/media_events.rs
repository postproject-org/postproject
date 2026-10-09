//! Original observations retain their source identity and typed media facts.

use postproject_core::{
    AssetId, LocatorId, MediaRootId, RepresentationId, ResourceId, RevisionEvent,
    RevisionEventKind, RevisionId,
};
use postproject_protocol::{Document, Limits, decode_event, encode_event};

#[test]
fn media_observations_retain_revision_position_and_every_exact_payload() {
    let asset_id = AssetId::new();
    let representation_id = RepresentationId::new();
    let resource_id = ResourceId::new();
    let locator_id = LocatorId::new();
    let media_root_id = MediaRootId::new();
    let events = [
        RevisionEventKind::AssetImported { asset_id },
        RevisionEventKind::RepresentationAdded {
            asset_id,
            representation_id,
        },
        RevisionEventKind::ResourceAdded { resource_id },
        RevisionEventKind::RepresentationResourceAdded {
            representation_id,
            resource_id,
            position: u32::MAX,
        },
        RevisionEventKind::LocatorAdded {
            resource_id,
            locator_id,
        },
        RevisionEventKind::LocatorRetired {
            resource_id,
            locator_id,
        },
        RevisionEventKind::MediaRootAdded { media_root_id },
        RevisionEventKind::MediaRootEnabledChanged {
            media_root_id,
            enabled: false,
        },
        RevisionEventKind::MediaRootEnabledChanged {
            media_root_id,
            enabled: true,
        },
        RevisionEventKind::MediaRootRemoved { media_root_id },
        RevisionEventKind::ResourceFileFactsObserved { resource_id },
    ];
    for kind in events {
        let event = RevisionEvent::new(RevisionId::new(), u32::MAX, kind);
        let bytes = encode_event(&event).unwrap().canonical_bytes().unwrap();
        assert_eq!(
            decode_event(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            event
        );
    }
}

#[test]
fn decoded_observations_reject_unknown_fields_invalid_positions_and_fake_booleans() {
    let event = RevisionEvent::new(
        RevisionId::new(),
        0,
        RevisionEventKind::MediaRootEnabledChanged {
            media_root_id: MediaRootId::new(),
            enabled: true,
        },
    );
    let source: serde_json::Value =
        serde_json::from_slice(&encode_event(&event).unwrap().canonical_bytes().unwrap()).unwrap();
    for (path, invalid) in [
        ("/position", serde_json::json!("4294967296")),
        ("/position", serde_json::json!("00")),
        ("/event/enabled", serde_json::json!("true")),
        ("/event/media_root_id", serde_json::json!("missing")),
    ] {
        let mut fields = source.clone();
        *fields.pointer_mut(path).unwrap() = invalid;
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| decode_event(&document))
                .is_err()
        );
    }
    let mut fields = source;
    fields["event"]["local_revision"] = "1".into();
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| decode_event(&document))
            .is_err()
    );
}
