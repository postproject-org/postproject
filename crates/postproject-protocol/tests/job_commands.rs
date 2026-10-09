//! Job intent is distinct from recorded state and separately supplied ownership.

use std::time::Duration;

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, AssetId, ContentStructure,
    Job, JobFailure, JobId, JobKind, JobState, Locator, LocatorAvailability, LocatorId,
    ProductionId, Representation, RepresentationId, RepresentationImport, RepresentationKind,
    RequestedJobOutput, Resource, ResourceId, RevisionContext, ToolIdentity,
};
use postproject_protocol::{
    ClientId, Command, Document, Extensions, FailureKind, HistoryId, Limits, Proposal,
    RecordFeature, RequestId, Scope,
};

fn commands() -> Vec<Command> {
    let asset = AssetId::new();
    let input = RepresentationId::new();
    let output = RepresentationId::new();
    let resource = ResourceId::new();
    let job = Job::new(
        JobId::new(),
        JobKind::new("unknown:Work").unwrap(),
        vec![input],
        RequestedJobOutput::new(asset, RepresentationKind::Proxy, None).unwrap(),
    )
    .unwrap();
    let media = RepresentationImport::new(
        Representation::new(
            output,
            asset,
            RepresentationKind::Proxy,
            ContentStructure::single_resource(resource),
            vec![],
        ),
        vec![Resource::new(resource, vec![], None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "file:///missing/proxy.mov",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Build").unwrap(),
        vec![ActivityInput::new(input, None)],
        vec![ActivityOutput::new(output, None)],
    )
    .unwrap();
    vec![
        Command::RequestJob(job.clone()),
        Command::ClaimJob {
            job_id: job.id(),
            tool: ToolIdentity::new("  Exact  ", None, None).unwrap(),
            agent: None,
            duration: Duration::from_micros(1),
        },
        Command::RenewJob {
            job_id: job.id(),
            duration: Duration::from_secs(86_400),
        },
        Command::ReleaseJob(job.id()),
        Command::FailJob {
            job_id: job.id(),
            failure: JobFailure::new("  failure 名  ").unwrap(),
        },
        Command::CompleteJob {
            job_id: job.id(),
            output: Box::new(media),
            activity: Box::new(activity),
        },
        Command::CancelJob(job.id()),
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

fn value(command: &Command) -> serde_json::Value {
    serde_json::from_slice(&command.document().unwrap().canonical_bytes().unwrap()).unwrap()
}

fn reject(value: &serde_json::Value) {
    let document = Document::parse(&serde_json::to_vec(value).unwrap(), Limits::default()).unwrap();
    assert!(Command::from_document(&document).is_err());
}

#[test]
fn all_work_intent_round_trips_without_secret_or_authority_clock_fields() {
    for command in commands() {
        assert_eq!(command.required_feature(), RecordFeature::Jobs);
        let bytes = command.document().unwrap().canonical_bytes().unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        for forbidden in [
            "claim_id",
            "expires_at_micros",
            "authority_time",
            "observed_revision",
        ] {
            assert!(!text.contains(forbidden));
        }
        assert_eq!(
            Command::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            command
        );
        let original = proposal(vec![command]);
        assert_eq!(
            Proposal::from_document(&original.document().unwrap()).unwrap(),
            original
        );
    }
    let mut commands = commands();
    let completion = commands.remove(5);
    assert_eq!(
        proposal(vec![completion])
            .required_features()
            .into_iter()
            .collect::<Vec<_>>(),
        [
            RecordFeature::Jobs,
            RecordFeature::Media,
            RecordFeature::Provenance
        ]
    );
    assert_eq!(
        proposal(commands)
            .required_features()
            .into_iter()
            .collect::<Vec<_>>(),
        [RecordFeature::Jobs]
    );
}

#[test]
fn privileged_state_authority_clock_and_bearer_fields_reject_in_every_intent() {
    for command in commands() {
        for field in [
            "claim_id",
            "expires_at_micros",
            "authority_time_micros",
            "input_boundary",
            "state",
        ] {
            let mut injected = value(&command);
            injected[field] = "1".into();
            reject(&injected);
        }
    }
    let original = value(&commands().remove(0));
    let mut cancelled = original.clone();
    cancelled["header"]["state"] = serde_json::json!({"kind":"cancelled"});
    reject(&cancelled);
    let mut duplicate = original;
    let input = duplicate["inputs"][0].clone();
    duplicate["inputs"].as_array_mut().unwrap().push(input);
    duplicate["header"]["input_count"] = "2".into();
    reject(&duplicate);
}

#[test]
fn duration_and_typed_requested_state_checks_are_exact() {
    let command = commands().remove(2);
    for duration in ["0", "86400000001", "-1", "01", "1.5"] {
        let mut invalid = value(&command);
        invalid["duration_micros"] = duration.into();
        reject(&invalid);
    }
    for duration in [Duration::ZERO, Duration::from_nanos(1), Duration::MAX] {
        assert_eq!(
            Command::RenewJob {
                job_id: JobId::new(),
                duration
            }
            .document()
            .unwrap_err()
            .kind(),
            FailureKind::Malformed
        );
    }
    let Command::RequestJob(job) = commands().remove(0) else {
        unreachable!()
    };
    assert_eq!(
        Command::RequestJob(job.with_state(JobState::Cancelled))
            .document()
            .unwrap_err()
            .kind(),
        FailureKind::Malformed
    );
}
