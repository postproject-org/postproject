//! Distinct checkpoint bases retain original activity evidence and query results.

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ArtifactEvaluationLimits,
    Asset, AssetId, ContentStructure, ExternalIdentifier, IdentifierScheme, Locator,
    LocatorAvailability, LocatorId, MetadataProperty, MetadataValue, ObjectRef,
    OriginalMediaImport, PropertyId, Representation, RepresentationFingerprint, RepresentationId,
    RepresentationKind, Resource, ResourceId, Timestamp, VocabularyId,
};
use postproject_storage_sqlite::{CheckpointLimits, ReplayLimits, SqliteProduction};

#[test]
fn early_checkpoint_suffix_and_later_checkpoint_keep_original_activity_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let imports = [media(), media()];
    let mut edit = source.begin_transaction().unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    let mut early = checkpoint(&source, &directory.path().join("early.pproj"));
    let input = imports[0].representation().id();
    let output = imports[1].representation().id();
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Render").unwrap(),
        vec![ActivityInput::new(input, None)],
        vec![ActivityOutput::new(output, None)],
    )
    .unwrap();
    let property = MetadataProperty::new(
        VocabularyId::new("unknown:CASE").unwrap(),
        PropertyId::new("Repeated").unwrap(),
    );
    let target = ObjectRef::Activity(activity.id());
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("unknown:CASE").unwrap(),
        " Exact 名 ",
        Some(" Qual ".into()),
    )
    .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.create_activity(&activity).unwrap();
    edit.record_representation_fingerprint(
        input,
        &RepresentationFingerprint::new("unknown_CASE", 1, vec![2]).unwrap(),
    )
    .unwrap();
    for _ in 0..2 {
        edit.add_metadata_value(target, &property, &MetadataValue::u64(u64::MAX))
            .unwrap();
    }
    edit.add_external_identifier(target, &identifier).unwrap();
    edit.commit().unwrap();
    drop(edit);
    apply(&source, &mut early, 2);
    let late_path = directory.path().join("late.pproj");
    let mut late = checkpoint(&source, &late_path);
    for mirror in [&early, &late] {
        compare(&source, mirror, output, target, &property);
    }
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_representation_fingerprint(
        input,
        &RepresentationFingerprint::new("unknown_CASE", 1, vec![3]).unwrap(),
    )
    .unwrap();
    edit.remove_external_identifier(target, &identifier)
        .unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(i64::MIN))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    for mirror in [&mut early, &mut late] {
        apply(&source, mirror, 3);
        compare(&source, mirror, output, target, &property);
    }
    let original = source.activities().unwrap().remove(0);
    assert_eq!(
        original.inputs()[0].snapshot().unwrap().revision_sequence(),
        2
    );
    assert_eq!(
        original.inputs()[0].snapshot().unwrap().fingerprints()[0].value(),
        [1]
    );
    drop(late);
    compare(
        &source,
        &SqliteProduction::open(late_path).unwrap(),
        output,
        target,
        &property,
    );
}

fn compare(
    source: &SqliteProduction,
    mirror: &SqliteProduction,
    output: RepresentationId,
    target: ObjectRef,
    property: &MetadataProperty,
) {
    assert_eq!(source.activities().unwrap(), mirror.activities().unwrap());
    assert_eq!(
        source.activities_producing(output).unwrap(),
        mirror.activities_producing(output).unwrap()
    );
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
    assert_eq!(
        source.metadata_values(target, property).unwrap(),
        mirror.metadata_values(target, property).unwrap()
    );
    assert_eq!(
        source.external_identifiers(target).unwrap(),
        mirror.external_identifiers(target).unwrap()
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
            vec![RepresentationFingerprint::new("unknown_CASE", 1, vec![1]).unwrap()],
        ),
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "file:///does-not-exist",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
