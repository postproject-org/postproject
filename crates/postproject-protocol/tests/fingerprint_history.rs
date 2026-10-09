//! Current and superseded domains remain distinct without storage row IDs.

use postproject_core::{AssetId, FingerprintSnapshot, ObjectRef, RepresentationId, ResourceId};
use postproject_protocol::{Document, FingerprintObservation, FingerprintState, Limits};

#[test]
fn current_and_same_revision_historical_evidence_round_trip_in_both_domains() {
    for target in [
        ObjectRef::Resource(ResourceId::new()),
        ObjectRef::Representation(RepresentationId::new()),
    ] {
        for observed in [None, Some(7)] {
            for state in [
                FingerprintState::Current,
                FingerprintState::Superseded {
                    position: 1,
                    revision_sequence: 7,
                },
            ] {
                let fact = FingerprintObservation::new(
                    target,
                    FingerprintSnapshot::new("unknown-domain", 0, vec![0, 255], observed).unwrap(),
                    state,
                )
                .unwrap();
                let bytes = fact.document().unwrap().canonical_bytes().unwrap();
                assert_eq!(
                    FingerprintObservation::from_document(
                        &Document::parse(&bytes, Limits::default()).unwrap()
                    )
                    .unwrap(),
                    fact
                );
                let fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                assert!(fields.get("row_id").is_none());
            }
        }
    }
}

#[test]
fn invalid_evidence_owners_and_historical_boundaries_reject() {
    let target = ObjectRef::Resource(ResourceId::new());
    let snapshot = FingerprintSnapshot::new("valid", 1, vec![1], Some(7)).unwrap();
    assert!(
        FingerprintObservation::new(
            ObjectRef::Asset(AssetId::new()),
            snapshot.clone(),
            FingerprintState::Current
        )
        .is_err()
    );
    for (position, revision_sequence) in [(0, 0), (0, 6), (0, u64::MAX), (u64::MAX, 7)] {
        assert!(
            FingerprintObservation::new(
                target,
                snapshot.clone(),
                FingerprintState::Superseded {
                    position,
                    revision_sequence
                }
            )
            .is_err()
        );
    }
    let fact = FingerprintObservation::new(target, snapshot, FingerprintState::Current).unwrap();
    let mut fields: serde_json::Value =
        serde_json::from_slice(&fact.document().unwrap().canonical_bytes().unwrap()).unwrap();
    fields["state"]["position"] = "0".into();
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| FingerprintObservation::from_document(&document))
            .is_err()
    );
}
