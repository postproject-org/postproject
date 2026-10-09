use super::*;
use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, Dependency, DependencyKind,
    DependencyTarget, ErrorKind, RepresentationFingerprint, ResourceFingerprint,
};
use postproject_protocol::RejectionKind;

fn fingerprint(id: RepresentationId, bytes: Vec<u8>) -> Command {
    Command::RecordRepresentationFingerprint {
        representation_id: id,
        fingerprint: RepresentationFingerprint::new("Unknown_P", 42, bytes).unwrap(),
    }
}

fn activity(input: RepresentationId, output: RepresentationId) -> Activity {
    Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Build").unwrap(),
        vec![ActivityInput::new(input, None)],
        vec![ActivityOutput::new(output, None)],
    )
    .unwrap()
}

#[test]
fn ordered_observations_capture_activity_evidence_before_later_fingerprint_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let input = prepared_import();
    let output = prepared_import();
    let leaf = prepared_import();
    let imports = proposal(
        &source,
        None,
        vec![
            Command::ImportOriginal(input.clone()),
            Command::ImportOriginal(output.clone()),
            Command::ImportOriginal(leaf.clone()),
        ],
    );
    source.submit_proposal(&imports).unwrap();
    let dependency = Dependency::new(
        None,
        DependencyKind::new("unknown:Exact").unwrap(),
        DependencyTarget::Representation(leaf.representation().id()),
        None,
        true,
        "  名  ",
    )
    .unwrap();
    let build = activity(input.representation().id(), output.representation().id());
    let base = source.read_session().unwrap().decision_base();
    let request = proposal(
        &source,
        Some(base),
        vec![
            Command::RecordDependencySet {
                representation_id: input.representation().id(),
                dependencies: vec![dependency.clone(), dependency.clone()],
            },
            Command::RecordResourceFingerprint {
                resource_id: input.resources()[0].id(),
                fingerprint: ResourceFingerprint::new("Unknown_R", 0, vec![0, 255]).unwrap(),
            },
            fingerprint(input.representation().id(), vec![1]),
            Command::RecordDependencySet {
                representation_id: input.representation().id(),
                dependencies: vec![dependency.clone(), dependency],
            },
            Command::CreateActivity(build.clone()),
            fingerprint(input.representation().id(), vec![2]),
        ],
    );
    let outcome = source.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Accepted(receipt)
        if receipt.revision().unwrap().sequence() == 2),
        "{outcome:?}"
    );
    let captured = source.activities().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].id(), build.id());
    let snapshot = captured[0].inputs()[0].snapshot().unwrap();
    assert_eq!(snapshot.fingerprints()[0].value(), [1]);
    assert_eq!(
        source
            .representation(input.representation().id())
            .unwrap()
            .fingerprints()[0]
            .value(),
        [2]
    );
    replay(&source, &mut mirror);
    assert_eq!(mirror.activities().unwrap(), captured);
    assert_eq!(
        mirror.dependency_set(input.representation().id()).unwrap(),
        source.dependency_set(input.representation().id()).unwrap()
    );
    assert_eq!(
        mirror.resources(input.representation().id()).unwrap(),
        source.resources(input.representation().id()).unwrap()
    );
    assert_eq!(
        mirror.representation(input.representation().id()).unwrap(),
        source.representation(input.representation().id()).unwrap()
    );
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    assert_eq!(source.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(source.changes_since(0, 10).unwrap().len(), 2);
}

#[test]
fn failed_activity_discards_fingerprint_and_dependency_changes_and_retains_rejection() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let (_, representation) = import(&mut source);
    let base = source.read_session().unwrap().decision_base();
    let request = proposal(
        &source,
        Some(base),
        vec![
            fingerprint(representation, vec![1]),
            Command::RecordDependencySet {
                representation_id: representation,
                dependencies: vec![],
            },
            Command::CreateActivity(activity(representation, RepresentationId::new())),
        ],
    );
    let head = source.exchange_head().unwrap();
    let outcome = source.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Rejected(rejection)
        if rejection.kind() == RejectionKind::Domain(ErrorKind::NotFound)),
        "{outcome:?}"
    );
    assert_eq!(source.exchange_head().unwrap(), head);
    assert!(
        source
            .representation(representation)
            .unwrap()
            .fingerprints()
            .is_empty()
    );
    assert!(source.dependency_set(representation).unwrap().is_none());
    assert!(source.activities().unwrap().is_empty());
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    assert_eq!(source.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(source.changes_since(0, 10).unwrap().len(), 1);
}
