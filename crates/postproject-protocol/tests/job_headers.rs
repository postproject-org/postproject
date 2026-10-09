//! Job scalar observations carry attribution and expiry, never credentials.

use postproject_core::{
    ActivityId, AgentIdentity, AssetId, ExternalIdentifier, IdentifierScheme, Job, JobClaim,
    JobCompletion, JobFailure, JobId, JobKind, JobState, RepresentationId, RepresentationKind,
    RequestedJobOutput, Timestamp, ToolIdentity,
};
use postproject_protocol::{Document, JobHeader, JobInput, Limits};

fn output() -> RequestedJobOutput {
    RequestedJobOutput::new(
        AssetId::new(),
        RepresentationKind::Optimized,
        Some("deliveries".into()),
    )
    .unwrap()
}

fn claim(expiry: i64) -> JobState {
    JobState::Claimed(JobClaim::new(
        ToolIdentity::new(
            "  Exact tool  ",
            Some("custom".into()),
            Some("https://example.com/tool".into()),
        )
        .unwrap(),
        Some(
            AgentIdentity::new(
                None,
                Some(
                    ExternalIdentifier::new(
                        IdentifierScheme::new("unknown.agent").unwrap(),
                        "  镜头 artist  ",
                        Some("exact".into()),
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        ),
        Timestamp::from_unix_micros(expiry),
    ))
}

#[test]
fn every_observed_state_and_exact_expiry_round_trips() {
    let states = [
        JobState::Requested,
        JobState::Cancelled,
        JobState::Succeeded(JobCompletion::new(
            ActivityId::new(),
            RepresentationId::new(),
        )),
        JobState::Failed(JobFailure::new("  exact diagnostic 镜头  ").unwrap()),
        claim(i64::MIN),
        claim(0),
        claim(9_007_199_254_740_993),
        claim(i64::MAX),
    ];
    for state in states {
        for count in [0, 100_000] {
            let header = JobHeader::new(
                JobId::new(),
                JobKind::new("unknown:Exact").unwrap(),
                count,
                output(),
                state.clone(),
            )
            .unwrap();
            let bytes = header.document().unwrap().canonical_bytes().unwrap();
            let restored =
                JobHeader::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                    .unwrap();
            assert_eq!(restored, header);
            assert_eq!(restored.input_count(), count);
            assert_eq!(restored.state(), &state);
        }
    }
    let job = Job::new(
        JobId::new(),
        JobKind::new("unknown:Request").unwrap(),
        vec![RepresentationId::new(), RepresentationId::new()],
        output(),
    )
    .unwrap();
    let copied = JobHeader::from_job(&job).unwrap();
    assert_eq!(copied.id(), job.id());
    assert_eq!(copied.input_count(), 2);
    assert_eq!(copied.requested_output(), job.requested_output());
    assert_eq!(copied.kind(), job.kind());
}

#[test]
fn input_identities_retain_the_native_position_bound() {
    for position in [0, 99_999] {
        let input = JobInput::new(JobId::new(), position, RepresentationId::new()).unwrap();
        assert_eq!(JobInput::from_document(&input.document()).unwrap(), input);
        assert_eq!(input.position(), position);
    }
    assert!(JobInput::new(JobId::new(), 100_000, RepresentationId::new()).is_err());
    let input = JobInput::new(JobId::new(), 0, RepresentationId::new()).unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&input.document().canonical_bytes().unwrap()).unwrap();
    for (field, value) in [
        ("position", "-1"),
        ("position", "00"),
        ("job_id", "invalid"),
        ("representation_id", "invalid"),
    ] {
        let mut changed = document.clone();
        changed[field] = value.into();
        assert!(
            Document::parse(&serde_json::to_vec(&changed).unwrap(), Limits::default())
                .and_then(|document| JobInput::from_document(&document))
                .is_err()
        );
    }
}

#[test]
fn credentials_and_contradictory_state_fields_are_rejected() {
    let header = JobHeader::new(
        JobId::new(),
        JobKind::new("unknown:Exact").unwrap(),
        1,
        output(),
        claim(123),
    )
    .unwrap();
    let original: serde_json::Value =
        serde_json::from_slice(&header.document().unwrap().canonical_bytes().unwrap()).unwrap();
    let cases: [(&[&str], serde_json::Value); 11] = [
        (&["input_count"], "100001".into()),
        (&["job_kind"], "not-namespaced".into()),
        (&["state", "kind"], "future".into()),
        (
            &["state", "claim_id"],
            postproject_core::JobClaimId::new().to_string().into(),
        ),
        (&["state", "tool"], serde_json::Value::Null),
        (
            &["state", "expires_at_micros"],
            "9223372036854775808".into(),
        ),
        (&["state", "expires_at_micros"], "00".into()),
        (&["requested_output", "asset_id"], "invalid".into()),
        (&["requested_output", "role"], "future".into()),
        (&["requested_output", "target_root"], "".into()),
        (&["unknown"], serde_json::Value::Null),
    ];
    for (fields, value) in cases {
        let mut changed = original.clone();
        let mut selected = &mut changed;
        for field in fields {
            selected = &mut selected[*field];
        }
        *selected = value;
        assert!(
            Document::parse(&serde_json::to_vec(&changed).unwrap(), Limits::default())
                .and_then(|document| JobHeader::from_document(&document))
                .is_err()
        );
    }
}
