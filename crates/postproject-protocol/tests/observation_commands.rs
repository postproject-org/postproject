//! Observation intent retains ordering and rejects storage-captured evidence.

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityRole, AssetId,
    Dependency, DependencyKind, DependencyTarget, ProductionId, RepresentationFingerprint,
    RepresentationId, ResourceFingerprint, ResourceId, RevisionContext,
};
use postproject_protocol::{
    ClientId, Command, Document, Extensions, FailureKind, HistoryId, Limits, Proposal,
    RecordFeature, RequestId, Scope,
};

fn intents() -> Vec<Command> {
    let source = RepresentationId::new();
    let output = RepresentationId::new();
    let dependencies = [
        DependencyTarget::Asset(AssetId::new()),
        DependencyTarget::Representation(output),
    ]
    .into_iter()
    .map(|target| {
        Dependency::new(
            None,
            DependencyKind::new("Unknown:Exact").unwrap(),
            target,
            None,
            true,
            "  /名/../shot.%04d.exr  ",
        )
        .unwrap()
    })
    .collect::<Vec<_>>();
    vec![
        Command::RecordResourceFingerprint {
            resource_id: ResourceId::new(),
            fingerprint: ResourceFingerprint::new("Unknown_R", u16::MAX, vec![0, 255]).unwrap(),
        },
        Command::RecordRepresentationFingerprint {
            representation_id: source,
            fingerprint: RepresentationFingerprint::new("Unknown_P", 0, vec![255, 0]).unwrap(),
        },
        Command::RecordDependencySet {
            representation_id: source,
            dependencies: vec![
                dependencies[0].clone(),
                dependencies[1].clone(),
                dependencies[0].clone(),
            ],
        },
        Command::RecordDependencySet {
            representation_id: source,
            dependencies: vec![],
        },
        Command::CreateActivity(
            Activity::new(
                ActivityId::new(),
                ActivityKind::new("unknown:Create").unwrap(),
                vec![ActivityInput::new(
                    source,
                    Some(ActivityRole::new("unknown:Input").unwrap()),
                )],
                vec![ActivityOutput::new(output, None)],
            )
            .unwrap(),
        ),
    ]
}

fn value(command: &Command) -> serde_json::Value {
    serde_json::from_slice(&command.document().unwrap().canonical_bytes().unwrap()).unwrap()
}

fn reject(value: &serde_json::Value) {
    let document = Document::parse(&serde_json::to_vec(value).unwrap(), Limits::default()).unwrap();
    assert_eq!(
        Command::from_document(&document).unwrap_err().kind(),
        FailureKind::Malformed
    );
}

#[test]
fn exact_observations_and_empty_dependency_intent_round_trip_with_exact_features() {
    let commands = intents();
    for command in &commands {
        let bytes = command.document().unwrap().canonical_bytes().unwrap();
        assert!(
            !std::str::from_utf8(&bytes)
                .unwrap()
                .contains("observed_revision")
        );
        assert_eq!(
            Command::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            *command
        );
    }
    let proposal = Proposal::new(
        Scope::new(ProductionId::new(), HistoryId::new()),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        commands,
        Extensions::default(),
    )
    .unwrap();
    assert_eq!(
        proposal.required_features().into_iter().collect::<Vec<_>>(),
        [
            RecordFeature::Dependencies,
            RecordFeature::Media,
            RecordFeature::Provenance
        ]
    );
    assert_eq!(
        Proposal::from_document(&proposal.document().unwrap()).unwrap(),
        proposal
    );
}

#[test]
fn fingerprint_observation_boundaries_and_dependency_state_are_not_intent() {
    let commands = intents();
    for command in &commands[..2] {
        for key in ["observed_revision_sequence", "previous", "archive_position"] {
            let mut forged = value(command);
            forged["fingerprint"][key] = "1".into();
            reject(&forged);
        }
    }
    let original = value(&commands[2]);
    for key in ["recorded_revision_sequence", "status", "input_boundary"] {
        let mut forged = original.clone();
        forged[key] = "1".into();
        reject(&forged);
    }
    let mut wrong_owner = original.clone();
    wrong_owner["occurrences"][0]["source_representation_id"] =
        RepresentationId::new().to_string().into();
    reject(&wrong_owner);
    let mut wrong_order = original.clone();
    wrong_order["occurrences"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    reject(&wrong_order);
    let mut boolean = original;
    boolean["occurrences"][0]["dependency"]["required"] = "true".into();
    reject(&boolean);
}

#[test]
fn activity_input_snapshots_counts_duplicates_and_overlap_reject() {
    let original = value(intents().last().unwrap());
    let mut snapshot = original.clone();
    snapshot["inputs"][0]["snapshot"] = serde_json::json!([]);
    reject(&snapshot);
    let mut wrong_count = original.clone();
    wrong_count["header"]["input_count"] = "2".into();
    reject(&wrong_count);
    let mut duplicate = original.clone();
    duplicate["header"]["input_count"] = "2".into();
    let edge = duplicate["inputs"][0].clone();
    duplicate["inputs"].as_array_mut().unwrap().push(edge);
    reject(&duplicate);
    let mut overlap = original;
    overlap["outputs"][0]["representation_id"] = overlap["inputs"][0]["representation_id"].clone();
    reject(&overlap);
}

#[test]
fn typed_activity_evidence_is_rejected_instead_of_discarded_during_normalization() {
    let input = ActivityInput::new(RepresentationId::new(), None)
        .with_snapshot(postproject_core::ActivityEdgeSnapshot::new(1, vec![]).unwrap());
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Build").unwrap(),
        vec![input],
        vec![ActivityOutput::new(RepresentationId::new(), None)],
    )
    .unwrap();
    assert_eq!(
        Command::CreateActivity(activity)
            .document()
            .unwrap_err()
            .kind(),
        FailureKind::Malformed
    );
}
