use postproject_core::{
    Asset, AssetId, ContentStructure, Dependency, DependencyKind, DependencySetStatus,
    DependencyTarget, Locator, LocatorAvailability, LocatorId, OriginalMediaImport, Representation,
    RepresentationId, RepresentationKind, Resource, ResourceId, Timestamp,
};
use postproject_protocol::{DependencyOccurrence, DependencySetHeader};

use crate::SqliteProduction;

use super::{Replacement, create, finish, invalidated};

fn fixture(directory: &std::path::Path) -> (SqliteProduction, RepresentationId, Dependency) {
    let mut source = SqliteProduction::create(directory.join("source.pproj"), None).unwrap();
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(0), None, None);
    let owner = RepresentationId::new();
    let resource = ResourceId::new();
    let import = OriginalMediaImport::new(
        asset.clone(),
        Representation::new(
            owner,
            asset.id(),
            RepresentationKind::Original,
            ContentStructure::single_resource(resource),
            Vec::new(),
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
    .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let dependency = Dependency::new(
        Some(resource),
        DependencyKind::new("unknown:Exact").unwrap(),
        DependencyTarget::Representation(owner),
        None,
        true,
        "  名/EXACT  ",
    )
    .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_dependency_set(owner, std::slice::from_ref(&dependency))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    (source, owner, dependency)
}

fn initialize(connection: &rusqlite::Connection) {
    super::super::media_state::create(connection).unwrap();
    create(connection).unwrap();
}

fn replacement(connection: &rusqlite::Connection, owner: RepresentationId, dependency: Dependency) {
    let header = DependencySetHeader::new(owner, 2, DependencySetStatus::Current, 1).unwrap();
    let mut pending = Replacement::begin(connection, header, 2, 1).unwrap();
    assert!(
        pending
            .push(
                connection,
                &DependencyOccurrence::new(owner, 0, dependency)
                    .unwrap()
                    .document()
                    .unwrap()
            )
            .unwrap()
    );
    pending.finish(connection).unwrap();
}

#[test]
fn duplicate_invalidation_and_unchanged_claimed_replacement_reject() {
    let directory = tempfile::tempdir().unwrap();
    let (source, owner, dependency) = fixture(directory.path());
    let transaction = source.connection.unchecked_transaction().unwrap();
    initialize(&transaction);
    replacement(&transaction, owner, dependency.clone());
    let header = DependencySetHeader::new(owner, 2, DependencySetStatus::Current, 1).unwrap();
    let mut duplicate = Replacement::begin(&transaction, header, 2, 1).unwrap();
    assert!(
        duplicate
            .push(
                &transaction,
                &DependencyOccurrence::new(owner, 0, dependency.clone())
                    .unwrap()
                    .document()
                    .unwrap()
            )
            .unwrap()
    );
    assert!(duplicate.finish(&transaction).is_err());
    assert!(invalidated(&transaction, owner, false, 1).is_err());
    invalidated(&transaction, owner, true, 1).unwrap();
    assert!(invalidated(&transaction, owner, true, 1).is_err());
    invalidated(&transaction, owner, false, 1).unwrap();
    assert!(
        finish(&transaction, 1).is_err(),
        "final Current contradicts the authored invalidation"
    );
    transaction.rollback().unwrap();
    assert_eq!(
        source.dependency_set(owner).unwrap().unwrap().status(),
        DependencySetStatus::Current
    );
}

#[test]
fn changed_authored_text_and_missing_replacement_evidence_reject_final_current_state() {
    let directory = tempfile::tempdir().unwrap();
    let (source, owner, dependency) = fixture(directory.path());
    for changed in [false, true] {
        let transaction = source.connection.unchecked_transaction().unwrap();
        initialize(&transaction);
        if changed {
            let altered = Dependency::new(
                dependency.source_resource_id(),
                dependency.kind().clone(),
                dependency.target(),
                dependency.resolved_representation_id(),
                dependency.is_required(),
                "different",
            )
            .unwrap();
            replacement(&transaction, owner, altered);
        }
        assert!(finish(&transaction, 1).is_err());
        transaction.rollback().unwrap();
    }
}
