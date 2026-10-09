//! Public worker notifications retain job identity and exclude bearer claims.

use postproject_core::{
    JobId, RepresentationId, ResourceId, RevisionEvent, RevisionEventKind, RevisionId,
};
use postproject_protocol::{Document, Limits, decode_event, encode_event};

#[test]
fn worker_and_fingerprint_observations_retain_exact_public_facts() {
    let job_id = JobId::new();
    let events = [
        RevisionEventKind::JobRequested { job_id },
        RevisionEventKind::JobClaimed { job_id },
        RevisionEventKind::JobClaimRenewed { job_id },
        RevisionEventKind::JobClaimReleased { job_id },
        RevisionEventKind::JobSucceeded { job_id },
        RevisionEventKind::JobFailed { job_id },
        RevisionEventKind::JobCancelled { job_id },
        RevisionEventKind::ResourceFingerprintObserved {
            resource_id: ResourceId::new(),
            algorithm: "UNKNOWN_domain-1".into(),
            version: u16::MAX,
        },
        RevisionEventKind::RepresentationFingerprintObserved {
            representation_id: RepresentationId::new(),
            algorithm: "UNKNOWN_domain-2".into(),
            version: 0,
        },
    ];
    for kind in events {
        let event = RevisionEvent::new(RevisionId::new(), 9, kind);
        let bytes = encode_event(&event).unwrap().canonical_bytes().unwrap();
        assert_eq!(
            decode_event(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            event
        );
        let fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(fields["event"].get("claim_id").is_none());
        assert!(fields["event"].get("token").is_none());
    }
}

#[test]
fn malformed_evidence_domains_and_credentials_reject() {
    let event = RevisionEvent::new(
        RevisionId::new(),
        0,
        RevisionEventKind::ResourceFingerprintObserved {
            resource_id: ResourceId::new(),
            algorithm: "valid".into(),
            version: 1,
        },
    );
    let source: serde_json::Value =
        serde_json::from_slice(&encode_event(&event).unwrap().canonical_bytes().unwrap()).unwrap();
    for (key, value) in [
        ("algorithm", String::new()),
        ("algorithm", "space domain".into()),
        ("algorithm", "x".repeat(65)),
        ("version", "65536".into()),
        ("version", "-1".into()),
    ] {
        let mut fields = source.clone();
        fields["event"][key] = value.into();
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| decode_event(&document))
                .is_err()
        );
    }
    let invalid = RevisionEvent::new(
        RevisionId::new(),
        0,
        RevisionEventKind::ResourceFingerprintObserved {
            resource_id: ResourceId::new(),
            algorithm: "bad!".into(),
            version: 1,
        },
    );
    assert!(encode_event(&invalid).is_err());
    let claimed = RevisionEvent::new(
        RevisionId::new(),
        0,
        RevisionEventKind::JobClaimed {
            job_id: JobId::new(),
        },
    );
    let mut fields: serde_json::Value =
        serde_json::from_slice(&encode_event(&claimed).unwrap().canonical_bytes().unwrap())
            .unwrap();
    fields["event"]["claim_id"] = "secret".into();
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| decode_event(&document))
            .is_err()
    );
}
