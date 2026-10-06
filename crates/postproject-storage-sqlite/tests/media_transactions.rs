//! Integration coverage for atomic media and media-root persistence.

use std::fs;

use postproject_core::{
    ErrorKind, FrameRange, RationalRate, RepresentationKind, RevisionEventKind, SequenceNaming,
    TransactionState,
};
use postproject_media::{ImageSequenceSource, prepare_media_root, prepare_original_media};
use postproject_storage_sqlite::SqliteProduction;
use tempfile::tempdir;

#[test]
fn imported_original_and_root_survive_reopen() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let media_directory = directory.path().join("rushes");
    fs::create_dir(&media_directory).expect("create media directory");
    let media_path = media_directory.join("A001.mov");
    fs::write(&media_path, b"fixture media bytes").expect("write fixture media");

    let prepared = prepare_original_media(
        &media_path,
        Some("A001".to_owned()),
        Some("integration-test".to_owned()),
    )
    .expect("prepare import");
    let root = prepare_media_root(&media_directory, Some("Rushes".to_owned()), 10)
        .expect("prepare media root");
    let asset_id = prepared.asset().id();
    let representation_id = prepared.representation().id();
    let resource_id = prepared.resources()[0].id();
    let locator_id = prepared.locators()[0].id();
    let fingerprint = prepared.resources()[0].fingerprints()[0].clone();

    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    {
        let mut transaction = production.begin_transaction().expect("begin transaction");
        transaction
            .import_original(&prepared)
            .expect("stage original import");
        transaction
            .add_media_root(root.clone())
            .expect("stage media root");
        transaction.commit().expect("commit transaction");
        assert_eq!(transaction.state(), TransactionState::Committed);
        assert_eq!(
            transaction
                .commit()
                .expect_err("second commit must fail")
                .kind(),
            ErrorKind::Conflict
        );
    }
    assert_eq!(
        production.media_roots().unwrap(),
        std::slice::from_ref(&root)
    );
    drop(production);

    let reopened = SqliteProduction::open(&production_path).expect("reopen production");
    let assets = reopened.assets().expect("load assets");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].id(), asset_id);
    assert_eq!(assets[0].display_name(), Some("A001"));
    let representations = reopened
        .representations(asset_id)
        .expect("load representations");
    assert_eq!(representations.len(), 1);
    assert_eq!(representations[0].id(), representation_id);
    assert_eq!(
        representations[0].content_structure().single_resource_id(),
        Some(resource_id)
    );
    let resources = reopened
        .resources(representation_id)
        .expect("load resources");
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].fingerprints(), [fingerprint]);
    let locators = reopened.locators(resource_id).expect("load locators");
    assert_eq!(locators.len(), 1);
    assert_eq!(locators[0].id(), locator_id);
    assert_eq!(locators[0].uri(), prepared.locators()[0].uri());
    assert_eq!(reopened.media_roots().unwrap(), [root]);
}

#[test]
fn image_sequence_imports_as_the_only_original() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let strip_directory = directory.path().join("strip");
    fs::create_dir(&strip_directory).expect("create strip directory");
    for frame in 1..=3 {
        fs::write(
            strip_directory.join(format!("strip_{frame:04}.png")),
            format!("frame {frame}"),
        )
        .expect("write frame");
    }
    let frames = FrameRange::new(1, 3, 1).expect("frame range");
    let prepared = prepare_original_media(
        ImageSequenceSource::new(
            &strip_directory,
            SequenceNaming::new("strip_", ".png", 4).expect("pattern"),
            frames,
            RationalRate::new(24, 1).expect("rate"),
            Vec::new(),
        ),
        Some("Strip".to_owned()),
        None,
    )
    .expect("prepare sequence import");
    let asset_id = prepared.asset().id();

    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let mut transaction = production.begin_transaction().expect("begin transaction");
    transaction
        .import_original(&prepared)
        .expect("stage sequence import");
    transaction.commit().expect("commit sequence import");
    drop(transaction);
    drop(production);

    let reopened = SqliteProduction::open(&production_path).expect("reopen production");
    let representations = reopened
        .representations(asset_id)
        .expect("load representations");
    assert_eq!(representations.len(), 1);
    assert_eq!(representations[0].kind(), RepresentationKind::Original);
    let descriptor = representations[0]
        .content_structure()
        .image_sequence_descriptor()
        .expect("sequence structure");
    assert_eq!(descriptor.frames(), frames);
    let locators = reopened
        .locators(descriptor.resource_id())
        .expect("load sequence locators");
    assert_eq!(
        locators[0].sequence_naming().map(SequenceNaming::prefix),
        Some("strip_")
    );
}

#[test]
fn explicit_and_implicit_rollback_leave_no_partial_import() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let media_path = directory.path().join("clip.mov");
    fs::write(&media_path, b"fixture media bytes").expect("write fixture media");
    let prepared =
        prepare_original_media(&media_path, None, None).expect("prepare original import");
    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");

    {
        let mut transaction = production.begin_transaction().expect("begin transaction");
        transaction
            .import_original(&prepared)
            .expect("stage original import");
        transaction.rollback().expect("roll back transaction");
        assert_eq!(transaction.state(), TransactionState::RolledBack);
    }
    assert_eq!(production.assets().expect("load assets"), []);

    {
        let mut transaction = production.begin_transaction().expect("begin transaction");
        transaction
            .import_original(&prepared)
            .expect("stage original import");
    }
    assert_eq!(production.assets().expect("load assets"), []);
}

#[test]
fn duplicate_media_root_is_explicit_and_can_be_rolled_back() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let root = prepare_media_root(directory.path(), None, 0).expect("prepare root");
    let duplicate = prepare_media_root(directory.path(), None, 1).expect("prepare duplicate root");
    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");

    let mut transaction = production.begin_transaction().expect("begin transaction");
    transaction.add_media_root(root).expect("stage first root");
    let error = transaction
        .add_media_root(duplicate)
        .expect_err("duplicate URI must fail");
    assert_eq!(error.kind(), ErrorKind::AlreadyExists);
    transaction.rollback().expect("roll back transaction");
    drop(transaction);

    assert_eq!(production.media_roots().unwrap(), []);
}

#[test]
fn roots_and_locators_have_a_complete_lifecycle() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let media_path = directory.path().join("clip.mov");
    fs::write(&media_path, b"fixture media bytes").expect("write fixture media");
    let prepared =
        prepare_original_media(&media_path, None, None).expect("prepare original import");
    let resource_id = prepared.resources()[0].id();
    let locator_id = prepared.locators()[0].id();
    let root =
        prepare_media_root(directory.path(), Some("Media".to_owned()), 4).expect("prepare root");
    let root_id = root.id();
    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");

    {
        let mut transaction = production.begin_transaction().expect("begin import");
        transaction
            .import_original(&prepared)
            .expect("stage original import");
        transaction.add_media_root(root).expect("stage root");
        transaction.commit().expect("commit import");
    }
    {
        let base = production.read_session().unwrap().decision_base();
        let mut transaction = production.begin_edit(base).expect("begin lifecycle change");
        transaction
            .set_media_root_enabled(root_id, false)
            .expect("disable root");
        transaction
            .set_media_root_enabled(root_id, false)
            .expect("repeat disabled state");
        transaction
            .retire_locator(locator_id)
            .expect("retire locator");
        transaction.commit().expect("commit lifecycle change");
    }

    assert!(!production.media_roots().unwrap()[0].is_enabled());
    assert_eq!(production.locators(resource_id).expect("load locators"), []);
    let revision = production
        .latest_revision()
        .expect("load revision")
        .unwrap();
    let events = production
        .events_for_revision(revision.id())
        .expect("load lifecycle events");
    assert_eq!(events.len(), 2);
    assert!(matches!(
        events[0].kind(),
        RevisionEventKind::MediaRootEnabledChanged { media_root_id, enabled: false }
            if *media_root_id == root_id
    ));
    assert!(matches!(
        events[1].kind(),
        RevisionEventKind::LocatorRetired { resource_id: id, locator_id: retired }
            if *id == resource_id && *retired == locator_id
    ));

    {
        let base = production.read_session().unwrap().decision_base();
        let mut transaction = production.begin_edit(base).expect("begin root removal");
        transaction.remove_media_root(root_id).expect("remove root");
        transaction.commit().expect("commit root removal");
    }
    assert_eq!(production.media_roots().unwrap(), []);
    drop(production);

    let reopened = SqliteProduction::open(production_path).expect("reopen production");
    assert_eq!(reopened.media_roots().unwrap(), []);
    let revision = reopened.latest_revision().expect("load revision").unwrap();
    let events = reopened
        .events_for_revision(revision.id())
        .expect("load root-removal event");
    assert!(matches!(
        events.as_slice(),
        [event] if matches!(
            event.kind(),
            RevisionEventKind::MediaRootRemoved { media_root_id } if *media_root_id == root_id
        )
    ));
}
