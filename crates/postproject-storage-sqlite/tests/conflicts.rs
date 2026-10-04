//! Multi-handle semantic optimistic-concurrency coverage.

use postproject_core::{
    Asset, AssetId, ContentStructure, ErrorKind, Locator, LocatorAvailability, LocatorId,
    OriginalMediaImport, Representation, RepresentationId, RepresentationKind, Resource,
    ResourceId, RevisionId, SemanticConflictKey, Timestamp, TransactionState,
};
use postproject_storage_sqlite::SqliteProduction;

fn single_file(label: u8) -> OriginalMediaImport {
    let asset_id = AssetId::from_bytes([label; 16]);
    let representation_id = RepresentationId::from_bytes([label; 16]);
    let resource_id = ResourceId::from_bytes([label; 16]);
    OriginalMediaImport::new(
        Asset::new(
            asset_id,
            Timestamp::from_unix_micros(i64::from(label)),
            Some(format!("asset-{label}")),
            None,
        ),
        Representation::new(
            representation_id,
            asset_id,
            RepresentationKind::Original,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        ),
        vec![Resource::new(resource_id, Vec::new(), None)],
        vec![locator(label, resource_id, "original")],
    )
    .expect("single-file import")
}

fn locator(label: u8, resource_id: ResourceId, name: &str) -> Locator {
    Locator::new(
        LocatorId::from_bytes([label; 16]),
        resource_id,
        format!("file:///shared/{name}-{label}.mov"),
        None,
        LocatorAvailability::Online,
    )
    .expect("locator")
}

fn import(production: &mut SqliteProduction, media: &OriginalMediaImport) {
    let mut transaction = production.begin_transaction().expect("begin import");
    transaction.import_original(media).expect("stage import");
    transaction.commit().expect("commit import");
}

#[test]
fn stale_additive_transactions_from_one_base_both_commit() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("additive.pproj");
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    import(&mut production, &single_file(1));
    let base = production
        .latest_revision()
        .expect("read latest revision")
        .expect("import revision")
        .id();

    for label in [2, 3] {
        let mut transaction = production
            .begin_transaction_at(base)
            .expect("begin stale additive transaction");
        transaction
            .import_original(&single_file(label))
            .expect("stage independent import");
        transaction.commit().expect("commit independent import");
    }

    assert_eq!(production.assets().expect("load assets").len(), 3);
}

#[test]
fn stale_locator_write_rolls_back_with_structured_conflict() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("same-key.pproj");
    let imported = single_file(4);
    let resource_id = imported.resources()[0].id();
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    import(&mut production, &imported);
    let base = production
        .latest_revision()
        .expect("read latest revision")
        .expect("import revision");

    let first = locator(5, resource_id, "first");
    let mut transaction = production
        .begin_transaction_at(base.id())
        .expect("begin first writer");
    transaction
        .add_locator(&first)
        .expect("stage first locator");
    transaction.commit().expect("commit first locator");
    drop(transaction);
    let superseding = production
        .latest_revision()
        .expect("read latest revision")
        .expect("locator revision");

    let rejected = locator(6, resource_id, "rejected");
    let unrelated = single_file(7);
    let mut stale = production
        .begin_transaction_at(base.id())
        .expect("begin stale writer");
    stale
        .import_original(&unrelated)
        .expect("stage unrelated additive work");
    stale.add_locator(&rejected).expect("stage stale locator");
    let error = stale.commit().expect_err("same semantic key must conflict");

    assert_eq!(stale.state(), TransactionState::RolledBack);
    assert_eq!(error.kind(), ErrorKind::Conflict);
    let detail = error
        .transaction_conflict_detail()
        .expect("structured conflict detail");
    assert_eq!(detail.key(), &SemanticConflictKey::LocatorSet(resource_id));
    assert_eq!(detail.base_revision(), Some(base.id()));
    assert_eq!(detail.base_sequence(), base.sequence());
    assert_eq!(detail.superseding_revision(), superseding.id());
    assert_eq!(detail.superseding_sequence(), superseding.sequence());
    drop(stale);
    assert_eq!(
        production.locators(resource_id).expect("load locators"),
        [imported.locators()[0].clone(), first]
    );
    assert!(
        production
            .asset(unrelated.asset().id())
            .is_err_and(|error| error.kind() == ErrorKind::NotFound)
    );
}

#[test]
fn stale_writes_to_independent_locator_sets_both_commit() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("independent-keys.pproj");
    let first = single_file(8);
    let second = single_file(9);
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    {
        let mut transaction = production.begin_transaction().expect("begin imports");
        transaction.import_original(&first).expect("import first");
        transaction.import_original(&second).expect("import second");
        transaction.commit().expect("commit imports");
    }
    let base = production
        .latest_revision()
        .expect("read latest revision")
        .expect("import revision")
        .id();

    for (label, resource_id) in [
        (10, first.resources()[0].id()),
        (11, second.resources()[0].id()),
    ] {
        let mut transaction = production
            .begin_transaction_at(base)
            .expect("begin based transaction");
        transaction
            .add_locator(&locator(label, resource_id, "independent"))
            .expect("stage independent locator");
        transaction.commit().expect("commit independent locator");
    }

    assert_eq!(
        production
            .locators(first.resources()[0].id())
            .expect("load first locators")
            .len(),
        2
    );
    assert_eq!(
        production
            .locators(second.resources()[0].id())
            .expect("load second locators")
            .len(),
        2
    );
}

#[test]
fn transaction_base_must_be_a_durable_revision() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("missing-base.pproj");
    let mut production = SqliteProduction::create(&path, None).expect("create production");

    let error = production
        .begin_transaction_at(RevisionId::from_bytes([42; 16]))
        .err()
        .expect("missing base must fail");

    assert_eq!(error.kind(), ErrorKind::NotFound);
}
