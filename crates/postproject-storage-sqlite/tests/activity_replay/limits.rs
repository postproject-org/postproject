use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, DependencyTarget,
    RepresentationFingerprint,
};
use postproject_protocol::{ActivityPathHeader, ActivityPathStatus};
use postproject_storage_sqlite::SqliteProduction;

use super::support;

#[test]
fn no_input_and_incomplete_dependency_observations_converge() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let input = support::media(1, true);
    let output = support::media(2, false);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.import_original(&input).unwrap();
    edit.import_original(&output).unwrap();
    edit.record_dependency_set(input.representation().id(), &[])
        .unwrap();
    edit.record_representation_fingerprint(
        input.representation().id(),
        &RepresentationFingerprint::new("unknown_Exact", 42, vec![88]).unwrap(),
    )
    .unwrap();
    let generator = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Generator").unwrap(),
        Vec::new(),
        vec![ActivityOutput::new(input.representation().id(), None)],
    )
    .unwrap();
    edit.create_activity(&generator).unwrap();
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Render").unwrap(),
        vec![ActivityInput::new(input.representation().id(), None)],
        vec![ActivityOutput::new(output.representation().id(), None)],
    )
    .unwrap();
    edit.create_activity(&activity).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    support::apply(&source, &mut mirror, 1);
    assert_eq!(mirror.activities().unwrap(), source.activities().unwrap());
    let paths: Vec<_> = support::frames(&source, 1)
        .iter()
        .filter(|document| document.kind().unwrap() == "activity.dependency-path")
        .map(|document| ActivityPathHeader::from_document(document).unwrap())
        .collect();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].status(), ActivityPathStatus::NeedsExtraction);
    assert_eq!(paths[0].segment_count(), 0);
}

#[test]
fn fixed_capture_depth_and_representation_limits_survive_replay() {
    for (name, count, chain, expected) in [
        ("depth", 66, true, ActivityPathStatus::DepthTruncated),
        (
            "representations",
            1002,
            false,
            ActivityPathStatus::RepresentationsTruncated,
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut source =
            SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
        let imports: Vec<_> = (1..=count + 1)
            .map(|label| support::media(label, false))
            .collect();
        let base = source.read_session().unwrap().decision_base();
        let mut edit = source.begin_edit(base).unwrap();
        for import in &imports {
            edit.import_original(import).unwrap();
        }
        if chain {
            for pair in imports[..count as usize].windows(2) {
                edit.record_dependency_set(
                    pair[0].representation().id(),
                    &[support::dependency(
                        DependencyTarget::Representation(pair[1].representation().id()),
                        true,
                    )],
                )
                .unwrap();
            }
        } else {
            let dependencies: Vec<_> = imports[1..count as usize]
                .iter()
                .map(|import| {
                    support::dependency(
                        DependencyTarget::Representation(import.representation().id()),
                        true,
                    )
                })
                .collect();
            edit.record_dependency_set(imports[0].representation().id(), &dependencies)
                .unwrap();
        }
        let activity = Activity::new(
            ActivityId::new(),
            ActivityKind::new("unknown:Render").unwrap(),
            vec![ActivityInput::new(imports[0].representation().id(), None)],
            vec![ActivityOutput::new(
                imports[count as usize].representation().id(),
                None,
            )],
        )
        .unwrap();
        edit.create_activity(&activity).unwrap();
        edit.commit().unwrap();
        drop(edit);
        let mut mirror = SqliteProduction::create_genesis_mirror(
            directory.path().join("mirror.pproj"),
            source.production(),
            source.exchange_floor().unwrap(),
        )
        .unwrap();
        support::apply(&source, &mut mirror, 1);
        assert_eq!(
            mirror.activities().unwrap(),
            source.activities().unwrap(),
            "{name}"
        );
        assert!(
            support::frames(&source, 1)
                .iter()
                .filter(|document| document.kind().unwrap() == "activity.dependency-path")
                .any(|document| ActivityPathHeader::from_document(document)
                    .unwrap()
                    .status()
                    == expected),
            "{name}"
        );
    }
}
