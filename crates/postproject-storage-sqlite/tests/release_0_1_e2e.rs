//! Complete 0.1 release relocation, ambiguity, and persistence scenario.

use std::{collections::BTreeMap, fs};

use postproject_core::{AssetId, RepresentationId, ResourceResolutionState};
use postproject_media::{
    MediaResolver, prepare_confirmed_locator, prepare_media_root, prepare_original_media,
};
use postproject_storage_sqlite::SqliteProduction;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "keeping the ordered acceptance scenario in one test makes its state transitions auditable"
)]
fn relocation_workflow_handles_unique_and_ambiguous_media() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production_path = directory.path().join("production.pproj");
    let original_root = directory.path().join("original");
    fs::create_dir(&original_root).expect("create original root");
    let fixtures = [
        ("camera-a.mov", b"camera A unique media".as_slice()),
        ("camera-b.mov", b"camera B unique media".as_slice()),
        ("ambiguous.wav", b"ambiguous production audio".as_slice()),
    ];
    for (name, bytes) in fixtures {
        fs::write(original_root.join(name), bytes).expect("write media fixture");
    }

    let mut production =
        SqliteProduction::create(&production_path, Some("E2E production".to_owned()))
            .expect("create production");
    let prepared: Vec<_> = fixtures
        .iter()
        .map(|(name, _)| {
            prepare_original_media(original_root.join(name), Some((*name).to_owned()), None)
                .expect("prepare media import")
        })
        .collect();
    let identities: BTreeMap<AssetId, RepresentationId> = prepared
        .iter()
        .map(|import| (import.asset().id(), import.representation().id()))
        .collect();
    let mut transaction = production
        .begin_transaction()
        .expect("begin import transaction");
    for import in &prepared {
        transaction
            .import_original(import)
            .expect("stage media import");
    }
    transaction.commit().expect("commit media imports");
    drop(transaction);
    drop(production);

    let relocated_parent = directory.path().join("relocated");
    let relocated_media = relocated_parent.join("media");
    fs::create_dir(&relocated_parent).expect("create relocated parent");
    fs::rename(&original_root, &relocated_media).expect("move entire media directory");
    let duplicate_directory = relocated_parent.join("duplicate");
    fs::create_dir(&duplicate_directory).expect("create duplicate directory");
    fs::copy(
        relocated_media.join("ambiguous.wav"),
        duplicate_directory.join("ambiguous-copy.wav"),
    )
    .expect("create ambiguous candidate");

    let mut production = SqliteProduction::open(&production_path).expect("reopen moved production");
    let resolver = MediaResolver::default();
    for (asset_id, representation_id) in &identities {
        let representation = production
            .representations(*asset_id)
            .expect("load representation")
            .into_iter()
            .find(|item| item.id() == *representation_id)
            .expect("find representation");
        let resource = production
            .resources(*representation_id)
            .expect("load resources")
            .remove(0);
        let locators = production
            .locators(resource.id())
            .expect("load known locators");
        let resolution = resolver
            .resolve_resource(
                &resource,
                representation.content_structure(),
                &locators,
                &[],
                &[],
            )
            .expect("resolve without roots");
        assert_eq!(resolution.state(), ResourceResolutionState::Offline);
    }

    let root = prepare_media_root(&relocated_parent, Some("relocated".to_owned()), 0)
        .expect("prepare relocated root");
    let mut transaction = production
        .begin_transaction()
        .expect("begin root transaction");
    transaction.add_media_root(root).expect("stage media root");
    transaction.commit().expect("commit media root");
    drop(transaction);

    let mut confirmations = Vec::new();
    let mut ambiguous_count = 0;
    for asset in production.assets().expect("load assets") {
        for representation in production
            .representations(asset.id())
            .expect("load representations")
        {
            let resource = production
                .resources(representation.id())
                .expect("load resources")
                .remove(0);
            let locators = production.locators(resource.id()).expect("load locators");
            let resolution = resolver
                .resolve_resource(
                    &resource,
                    representation.content_structure(),
                    &locators,
                    &production.media_roots().unwrap(),
                    &[],
                )
                .expect("resolve relocated media");
            match resolution.state() {
                ResourceResolutionState::ResolvedExact => {
                    assert_eq!(resolution.candidates().len(), 1);
                }
                ResourceResolutionState::Ambiguous => {
                    ambiguous_count += 1;
                    assert_eq!(resolution.candidates().len(), 2);
                }
                state => panic!("unexpected relocation state: {state:?}"),
            }
            let selected = resolution
                .candidates()
                .first()
                .expect("resolved candidate exists");
            confirmations.push(
                prepare_confirmed_locator(resource.id(), selected.uri(), None, None)
                    .expect("prepare confirmed locator"),
            );
        }
    }
    assert_eq!(ambiguous_count, 1);

    let mut transaction = production
        .begin_transaction()
        .expect("begin confirmation transaction");
    for locator in &confirmations {
        transaction
            .add_locator(locator)
            .expect("stage confirmed locator");
    }
    transaction.commit().expect("commit confirmed locations");
    drop(transaction);
    drop(production);

    let production = SqliteProduction::open(&production_path).expect("reopen confirmed production");
    for (asset_id, representation_id) in identities {
        let representation = production
            .representations(asset_id)
            .expect("load representation")
            .into_iter()
            .find(|item| item.id() == representation_id)
            .expect("find representation");
        let resource = production
            .resources(representation_id)
            .expect("load persisted resources")
            .remove(0);
        let locators = production
            .locators(resource.id())
            .expect("load persisted locators");
        assert_eq!(locators.len(), 2);
        let resolution = resolver
            .resolve_resource(
                &resource,
                representation.content_structure(),
                &locators,
                &[],
                &[],
            )
            .expect("resolve from confirmed locator without roots");
        assert_eq!(
            resolution.state(),
            ResourceResolutionState::OnlineAtKnownLocator
        );
    }
}
