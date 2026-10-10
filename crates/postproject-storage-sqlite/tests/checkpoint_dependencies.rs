//! Checkpoints retain ordered dependencies and original publication evidence.

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ArtifactEvaluationLimits,
    Asset, AssetId, ContentStructure, Dependency, DependencyKind, DependencySetStatus,
    DependencyTarget, Locator, LocatorAvailability, LocatorId, OriginalMediaImport, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationKind, Resource, ResourceId,
    Timestamp,
};
use postproject_storage_sqlite::{CheckpointLimits, ReplayLimits, SqliteProduction};

#[test]
fn checkpoint_suffix_preserves_empty_repeated_dirty_sets_and_historical_paths() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let imports = [media(), media(), media()];
    let input = imports[0].representation().id();
    let middle = imports[1].representation().id();
    let output = imports[2].representation().id();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    let repeated = vec![
        Dependency::new(
            Some(imports[0].resources()[0].id()),
            DependencyKind::new("unknown:Reference").unwrap(),
            DependencyTarget::Asset(imports[1].asset().id()),
            Some(middle),
            true,
            "  名/../EXACT%2f  "
        )
        .unwrap();
        1001
    ];
    edit.record_dependency_set(input, &repeated).unwrap();
    edit.record_dependency_set(middle, &[]).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let early_path = directory.path().join("early.pproj");
    let mut early = checkpoint(&source, &early_path);
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Render").unwrap(),
        vec![ActivityInput::new(input, None)],
        vec![ActivityOutput::new(output, None)],
    )
    .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.create_activity(&activity).unwrap();
    edit.record_representation_fingerprint(
        middle,
        &RepresentationFingerprint::new("domain", 1, vec![2]).unwrap(),
    )
    .unwrap();
    edit.record_dependency_set(input, &[]).unwrap();
    edit.commit().unwrap();
    drop(edit);
    apply(&source, &mut early, 2);
    let late_path = directory.path().join("late.pproj");
    let mut late = checkpoint(&source, &late_path);
    compare(&source, &early, &[input, middle, output], output);
    compare(&source, &late, &[input, middle, output], output);
    assert_eq!(
        source
            .dependency_set(input)
            .unwrap()
            .unwrap()
            .dependencies(),
        []
    );
    assert_eq!(
        source.dependency_set(middle).unwrap().unwrap().status(),
        DependencySetStatus::NeedsExtraction
    );
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_dependency_set(input, &repeated).unwrap();
    edit.record_dependency_set(middle, &[]).unwrap();
    edit.record_representation_fingerprint(
        input,
        &RepresentationFingerprint::new("domain", 1, vec![3]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    for mirror in [&mut early, &mut late] {
        apply(&source, mirror, 3);
        compare(&source, mirror, &[input, middle, output], output);
    }
    drop(early);
    drop(late);
    for path in [early_path, late_path] {
        let mirror = SqliteProduction::open(path).unwrap();
        compare(&source, &mirror, &[input, middle, output], output);
    }
}

fn compare(
    source: &SqliteProduction,
    mirror: &SqliteProduction,
    owners: &[RepresentationId],
    output: RepresentationId,
) {
    for owner in owners {
        assert_eq!(
            source.dependency_set(*owner).unwrap(),
            mirror.dependency_set(*owner).unwrap()
        );
        assert_eq!(
            source.representation(*owner).unwrap(),
            mirror.representation(*owner).unwrap()
        );
    }
    assert_eq!(source.activities().unwrap(), mirror.activities().unwrap());
    assert_eq!(
        source
            .evaluate_artifact(output, ArtifactEvaluationLimits::default())
            .unwrap(),
        mirror
            .evaluate_artifact(output, ArtifactEvaluationLimits::default())
            .unwrap()
    );
    assert_eq!(
        source.artifact_reproducibility(output).unwrap(),
        mirror.artifact_reproducibility(output).unwrap()
    );
    let revisions = source.changes_since(0, 10).unwrap();
    assert_eq!(revisions, mirror.changes_since(0, 10).unwrap());
    for revision in revisions {
        assert_eq!(
            source.events_for_revision(revision.id()).unwrap(),
            mirror.events_for_revision(revision.id()).unwrap()
        );
    }
    assert_eq!(
        source.exchange_head().unwrap(),
        mirror.exchange_head().unwrap()
    );
}

fn checkpoint(source: &SqliteProduction, path: &std::path::Path) -> SqliteProduction {
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    SqliteProduction::import_checkpoint(
        path,
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap()
}

fn apply(source: &SqliteProduction, mirror: &mut SqliteProduction, sequence: u64) {
    let mut reader = source.record_reader(sequence).unwrap();
    let manifest = reader.manifest().clone();
    assert!(
        mirror
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
}

fn media() -> OriginalMediaImport {
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(-1), None, None);
    let resource = ResourceId::new();
    OriginalMediaImport::new(
        asset.clone(),
        Representation::new(
            RepresentationId::new(),
            asset.id(),
            RepresentationKind::Original,
            ContentStructure::single_resource(resource),
            vec![RepresentationFingerprint::new("domain", 1, vec![1]).unwrap()],
        ),
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "file:///missing",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
