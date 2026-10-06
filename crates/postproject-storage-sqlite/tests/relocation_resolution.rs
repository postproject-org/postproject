//! End-to-end relocation, ambiguity, confirmation, and reopen scenario.

use std::fs;

use postproject_core::{Locator, MediaRoot, ResourceResolutionState};
use postproject_media::{
    MediaResolver, prepare_confirmed_locator, prepare_media_root, prepare_original_media,
};
use postproject_storage_sqlite::SqliteProduction;
use tempfile::tempdir;

#[test]
fn moved_media_resolves_and_confirmed_location_persists() {
    let directory = tempdir().expect("create temporary directory");
    let production_path = directory.path().join("production.pproj");
    let original_directory = directory.path().join("original");
    let relocated_directory = directory.path().join("relocated");
    fs::create_dir(&original_directory).expect("create original directory");
    let original_path = original_directory.join("A001.mov");
    fs::write(&original_path, b"unique fixture media").expect("write media");
    let prepared = prepare_original_media(&original_path, Some("A001".to_owned()), None)
        .expect("prepare import");
    let asset_id = prepared.asset().id();

    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let mut transaction = production.begin_transaction().expect("begin import");
    transaction
        .import_original(&prepared)
        .expect("stage import");
    transaction.commit().expect("commit import");
    drop(transaction);
    drop(production);

    fs::rename(&original_directory, &relocated_directory).expect("move media directory");
    let relocated_path = relocated_directory.join("A001.mov");
    let resolver = MediaResolver::default();
    let mut production = SqliteProduction::open(&production_path).expect("reopen moved production");
    let representation = production
        .representations(asset_id)
        .expect("load representation")
        .remove(0);
    let resource = production
        .resources(representation.id())
        .expect("load resource")
        .remove(0);
    let known_locators = production
        .locators(resource.id())
        .expect("load known locators");
    let resolve = |locators: &[Locator], roots: &[MediaRoot]| {
        resolver.resolve_resource(
            &resource,
            representation.content_structure(),
            locators,
            roots,
            &[],
        )
    };
    let missing = resolve(&known_locators, &[]).expect("resolve without roots");
    assert_eq!(missing.state(), ResourceResolutionState::Offline);

    let root = prepare_media_root(&relocated_directory, None, 0).expect("prepare new root");
    let mut transaction = production
        .begin_transaction()
        .expect("begin root transaction");
    transaction
        .add_media_root(root)
        .expect("stage new media root");
    transaction.commit().expect("commit root");
    drop(transaction);

    let unique =
        resolve(&known_locators, &production.media_roots().unwrap()).expect("resolve unique media");
    assert_eq!(unique.state(), ResourceResolutionState::ResolvedExact);

    let duplicate_path = relocated_directory.join("duplicate.mov");
    fs::copy(&relocated_path, &duplicate_path).expect("create duplicate");
    let ambiguous = resolve(&known_locators, &production.media_roots().unwrap())
        .expect("resolve duplicate media");
    assert_eq!(ambiguous.state(), ResourceResolutionState::Ambiguous);
    assert_eq!(ambiguous.candidates().len(), 2);

    let chosen_uri = ambiguous
        .candidates()
        .iter()
        .find(|candidate| candidate.uri().ends_with("A001.mov"))
        .expect("find intended candidate")
        .uri()
        .to_owned();
    let confirmed = prepare_confirmed_locator(resource.id(), chosen_uri, None, None)
        .expect("prepare confirmed locator");
    let mut transaction = production
        .begin_transaction()
        .expect("begin confirmation transaction");
    transaction
        .add_locator(&confirmed)
        .expect("stage confirmed locator");
    transaction.commit().expect("commit confirmation");
    drop(transaction);
    drop(production);

    let reopened = SqliteProduction::open(&production_path).expect("reopen confirmed production");
    let locators = reopened
        .locators(resource.id())
        .expect("load confirmed locators");
    assert_eq!(locators.len(), 2);
    let online =
        resolve(&locators, &reopened.media_roots().unwrap()).expect("resolve confirmed locator");
    assert_eq!(
        online.state(),
        ResourceResolutionState::OnlineAtKnownLocator
    );
    assert_eq!(online.candidates()[0].uri(), confirmed.uri());
}
