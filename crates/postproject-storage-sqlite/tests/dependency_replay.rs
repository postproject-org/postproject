//! Complete authored dependency replacements converge in distinct passive files.

use postproject_core::{
    Dependency, DependencyKind, DependencySetStatus, DependencyTarget, OriginalMediaImport,
    RepresentationFingerprint,
};
use postproject_media::prepare_original_media;
use postproject_protocol::RecordFeature;
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

fn dependencies(import: &OriginalMediaImport) -> Vec<Dependency> {
    let edge = |target, resolved, required, authored| {
        Dependency::new(
            Some(import.resources()[0].id()),
            DependencyKind::new("unknown:ExAct").unwrap(),
            target,
            resolved,
            required,
            authored,
        )
        .unwrap()
    };
    vec![
        edge(
            DependencyTarget::Asset(import.asset().id()),
            Some(import.representation().id()),
            true,
            "  名/EXACT%2f  ",
        ),
        edge(
            DependencyTarget::Asset(import.asset().id()),
            None,
            false,
            "",
        ),
        edge(
            DependencyTarget::Representation(import.representation().id()),
            None,
            true,
            "self/pinned",
        ),
    ]
}

fn source(directory: &std::path::Path) -> (SqliteProduction, OriginalMediaImport) {
    let media = directory.join("clip.dat");
    std::fs::write(&media, b"dependency replay").unwrap();
    let import = prepare_original_media(&media, None, None).unwrap();
    let mut source = SqliteProduction::create(directory.join("source.pproj"), None).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.import_original(&import).unwrap();
    let representation = import.representation().id();
    assert!(edit.record_dependency_set(representation, &[]).unwrap());
    let repeated = vec![dependencies(&import)[0].clone(); 2];
    assert!(
        edit.record_dependency_set(representation, &repeated)
            .unwrap()
    );
    assert!(
        !edit
            .record_dependency_set(representation, &repeated)
            .unwrap()
    );
    assert!(edit.record_dependency_set(representation, &[]).unwrap());
    assert!(
        edit.record_dependency_set(representation, &dependencies(&import))
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    std::fs::remove_file(media).unwrap();
    (source, import)
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

#[test]
fn empty_repeated_floating_pinned_and_invalidated_observations_converge() {
    let directory = tempfile::tempdir().unwrap();
    let (mut source, import) = source(directory.path());
    let path = directory.path().join("mirror.pproj");
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let manifest = source.record_reader(1).unwrap().manifest().clone();
    assert_eq!(manifest.effect_count(), 5);
    assert_eq!(
        manifest.required_features().collect::<Vec<_>>(),
        [
            RecordFeature::Dependencies,
            RecordFeature::Media,
            RecordFeature::RecordChunks
        ]
    );
    apply(&source, &mut mirror, 1);
    let representation = import.representation().id();
    assert_eq!(
        mirror.dependency_set(representation).unwrap(),
        source.dependency_set(representation).unwrap()
    );
    let fingerprint = &import.representation().fingerprints()[0];
    let changed =
        RepresentationFingerprint::new(fingerprint.algorithm(), fingerprint.version(), vec![77])
            .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_representation_fingerprint(representation, &changed)
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    apply(&source, &mut mirror, 2);
    assert_eq!(
        mirror.dependency_set(representation).unwrap(),
        source.dependency_set(representation).unwrap()
    );
    assert_eq!(
        mirror
            .dependency_set(representation)
            .unwrap()
            .unwrap()
            .status(),
        DependencySetStatus::NeedsExtraction
    );
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_dependency_set(representation, &dependencies(&import))
            .unwrap()
    );
    edit.commit().unwrap();
    drop(edit);
    apply(&source, &mut mirror, 3);
    let expected = source.dependency_set(representation).unwrap();
    assert_eq!(mirror.dependency_set(representation).unwrap(), expected);
    assert_eq!(expected.unwrap().recorded_at_revision(), 3);
    for sequence in 1..=3 {
        let manifest = source.record_reader(sequence).unwrap().manifest().clone();
        assert_eq!(
            mirror
                .events_for_revision(manifest.revision().id())
                .unwrap(),
            source
                .events_for_revision(manifest.revision().id())
                .unwrap()
        );
    }
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    drop(mirror);
    let mut mirror = SqliteProduction::open(&path).unwrap();
    let mut reader = source.record_reader(3).unwrap();
    assert!(
        !mirror
            .apply_record(
                &reader.manifest().clone(),
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
}
