//! Passive reconstruction retains evidence from the original authored prefix.

#[path = "activity_replay/cycles.rs"]
mod cycles;
#[path = "activity_replay/integrity.rs"]
mod integrity;
#[path = "activity_replay/limits.rs"]
mod limits;
#[path = "activity_replay/support.rs"]
mod support;

use postproject_core::{
    Activity, ActivityEdgeSnapshot, ActivityId, ActivityInput, ActivityKind, ActivityOutput,
    ActivityRole, AgentIdentity, ArtifactEvaluationLimits, DependencyTarget, FingerprintSnapshot,
    RepresentationFingerprint, Timestamp, ToolIdentity,
};
use postproject_protocol::RecordFeature;
use postproject_storage_sqlite::SqliteProduction;

fn source(directory: &std::path::Path) -> (SqliteProduction, Activity) {
    let mut source = SqliteProduction::create(directory.join("source.pproj"), None).unwrap();
    let imports = [
        support::media(1, true),
        support::media(2, true),
        support::media(3, false),
        support::media(4, true),
    ];
    let representation = |index: usize| imports[index].representation().id();
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Render").unwrap(),
        vec![
            ActivityInput::new(representation(0), None).with_snapshot(
                ActivityEdgeSnapshot::new(
                    777,
                    vec![FingerprintSnapshot::new("caller", 1, vec![99], None).unwrap()],
                )
                .unwrap(),
            ),
            ActivityInput::new(
                representation(0),
                Some(ActivityRole::new("unknown:Input").unwrap()),
            ),
            ActivityInput::new(representation(2), None),
        ],
        vec![
            ActivityOutput::new(representation(3), None),
            ActivityOutput::new(
                representation(3),
                Some(ActivityRole::new("unknown:Output").unwrap()),
            ),
        ],
    )
    .unwrap()
    .with_timing(
        Some(Timestamp::from_unix_micros(-9_007_199_254_740_993)),
        Some(Timestamp::from_unix_micros(9_007_199_254_740_993)),
    )
    .unwrap()
    .with_tool(
        ToolIdentity::new(
            "  Exact tool  ",
            Some(" custom version ".into()),
            Some("https://example.com/tool".into()),
        )
        .unwrap(),
    )
    .with_agent(AgentIdentity::new(Some("镜头 artist".into()), None).unwrap());
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    edit.record_dependency_set(
        representation(0),
        &[
            support::dependency(DependencyTarget::Representation(representation(1)), true),
            support::dependency(DependencyTarget::Asset(imports[2].asset().id()), true),
            support::dependency(DependencyTarget::Representation(representation(3)), false),
        ],
    )
    .unwrap();
    edit.record_dependency_set(representation(1), &[]).unwrap();
    edit.create_activity(&activity).unwrap();
    for index in [0, 1, 3] {
        edit.record_representation_fingerprint(
            representation(index),
            &RepresentationFingerprint::new("unknown_Exact", 42, vec![77]).unwrap(),
        )
        .unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    (source, activity)
}

#[test]
fn original_snapshots_survive_later_same_transaction_observations_and_restart() {
    let directory = tempfile::tempdir().unwrap();
    let (source, authored) = source(directory.path());
    let manifest = source.record_reader(1).unwrap().manifest().clone();
    assert!(
        manifest
            .required_features()
            .any(|feature| feature == RecordFeature::Provenance)
    );
    let path = directory.path().join("mirror.pproj");
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    support::apply(&source, &mut mirror, 1);
    let captured = source.activities().unwrap().remove(0);
    assert_ne!(
        captured, authored,
        "native snapshots replace caller-supplied evidence"
    );
    assert_eq!(
        mirror.activities().unwrap(),
        std::slice::from_ref(&captured)
    );
    let input = &captured.inputs()[0];
    let snapshot = input.snapshot().unwrap();
    assert_eq!(snapshot.revision_sequence(), 1);
    assert_eq!(snapshot.fingerprints()[0].value(), 1_u32.to_be_bytes());
    let output = captured.outputs()[0].representation_id();
    assert_eq!(
        mirror
            .evaluate_artifact(output, ArtifactEvaluationLimits::default())
            .unwrap(),
        source
            .evaluate_artifact(output, ArtifactEvaluationLimits::default())
            .unwrap()
    );
    assert_eq!(
        mirror.artifact_reproducibility(output).unwrap(),
        source.artifact_reproducibility(output).unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    drop(mirror);
    let mut mirror = SqliteProduction::open(&path).unwrap();
    assert_eq!(mirror.activities().unwrap(), [captured]);
    let mut reader = source.record_reader(1).unwrap();
    assert!(
        !mirror
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                postproject_storage_sqlite::ReplayLimits::default()
            )
            .unwrap()
    );
}
