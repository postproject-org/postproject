use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityRole, Dependency,
    DependencyKind, DependencyTarget, ExternalIdentifier, IdentifierScheme, MetadataProperty,
    MetadataValue, ObjectRef, PropertyId, RepresentationFingerprint, VocabularyId,
};

use crate::SqliteProduction;

#[test]
fn retained_activity_preserves_its_original_fingerprints_and_pre_floor_baseline() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    super::media::populate(&mut source, None);
    super::media::populate(&mut source, None);
    let assets = source.assets().unwrap();
    let input = source.representations(assets[0].id()).unwrap()[0].id();
    let output = source.representations(assets[1].id()).unwrap()[0].id();
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
        input,
        &RepresentationFingerprint::new("aggregate", 1, vec![99]).unwrap(),
    )
    .unwrap();
    edit.add_metadata_value(
        ObjectRef::Activity(activity.id()),
        &MetadataProperty::new(
            VocabularyId::new("unknown:CASE").unwrap(),
            PropertyId::new("Repeated").unwrap(),
        ),
        &MetadataValue::u64(u64::MAX),
    )
    .unwrap();
    edit.add_external_identifier(
        ObjectRef::Activity(activity.id()),
        &ExternalIdentifier::new(
            IdentifierScheme::new("unknown:CASE").unwrap(),
            " Exact 名 ",
            None,
        )
        .unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    super::media::audit(&source, 0, 5);
    super::media::audit(&source, 4, 5);
    assert_eq!(
        source.activities().unwrap()[0].inputs()[0]
            .snapshot()
            .unwrap()
            .fingerprints()[0]
            .value(),
        [12]
    );
}

#[test]
fn paths_use_original_dependency_occurrences_and_fingerprints_after_later_replacements() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("paths.pproj"), None).unwrap();
    for _ in 0..3 {
        super::media::populate(&mut source, None);
    }
    let assets = source.assets().unwrap();
    let owner = |index: usize| source.representations(assets[index].id()).unwrap()[0].id();
    let input = owner(0);
    let middle = owner(1);
    let output = owner(2);
    let dependency = |target, required| {
        Dependency::new(
            None,
            DependencyKind::new("unknown:Exact").unwrap(),
            target,
            None,
            required,
            "  名/../EXACT%2f  ",
        )
        .unwrap()
    };
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_dependency_set(
        input,
        &[
            dependency(DependencyTarget::Representation(middle), true),
            dependency(DependencyTarget::Representation(output), false),
            dependency(DependencyTarget::Asset(assets[2].id()), true),
        ],
    )
    .unwrap();
    edit.record_dependency_set(
        middle,
        &[dependency(DependencyTarget::Representation(input), true)],
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Render").unwrap(),
        vec![
            ActivityInput::new(input, None),
            ActivityInput::new(input, Some(ActivityRole::new("unknown:Second").unwrap())),
        ],
        vec![ActivityOutput::new(output, None)],
    )
    .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.create_activity(&activity).unwrap();
    edit.record_representation_fingerprint(
        middle,
        &RepresentationFingerprint::new("aggregate", 1, vec![99]).unwrap(),
    )
    .unwrap();
    edit.record_dependency_set(input, &[]).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let paths: i64 = source
        .connection
        .query_row(
            "SELECT COUNT(*) FROM activity_input_dependency_paths",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        paths, 4,
        "both role-distinct inputs retain two original paths"
    );
    for floor in [0, 6, 7, 8] {
        super::media::audit(&source, floor, 8);
    }
}
