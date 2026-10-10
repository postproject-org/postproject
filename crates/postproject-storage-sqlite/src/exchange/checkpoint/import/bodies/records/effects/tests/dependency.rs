use postproject_core::{Dependency, DependencyKind, DependencyTarget, RepresentationFingerprint};

use crate::SqliteProduction;

use super::media;

#[test]
fn retained_dependency_replacements_and_invalidation_explain_genesis_and_unknown_baselines() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    media::populate(&mut source, None);
    let representation = source
        .representations(source.assets().unwrap()[0].id())
        .unwrap()[0]
        .id();
    let dependency = Dependency::new(
        None,
        DependencyKind::new("unknown:Exact").unwrap(),
        DependencyTarget::Representation(representation),
        None,
        true,
        "  名/../EXACT%2f  ",
    )
    .unwrap();
    let repeated = vec![dependency; 1001];
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(edit.record_dependency_set(representation, &[]).unwrap());
    assert!(
        edit.record_dependency_set(representation, &repeated)
            .unwrap()
    );
    assert!(
        !edit
            .record_dependency_set(representation, &repeated)
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_representation_fingerprint(
        representation,
        &RepresentationFingerprint::new("aggregate", 1, vec![13]).unwrap(),
    )
    .unwrap();
    assert!(
        edit.record_dependency_set(representation, &repeated)
            .unwrap()
    );
    edit.record_representation_fingerprint(
        representation,
        &RepresentationFingerprint::new("aggregate", 1, vec![14]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(edit.record_dependency_set(representation, &[]).unwrap());
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_dependency_set(representation, &repeated)
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_representation_fingerprint(
        representation,
        &RepresentationFingerprint::new("aggregate", 1, vec![15]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    for floor in [0, 2, 3, 4, 5, 6, 7] {
        media::audit(&source, floor, 7);
    }
}
