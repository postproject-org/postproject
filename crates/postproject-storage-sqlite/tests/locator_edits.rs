//! Destructive locator decisions require a base and roll back after conflict.

use postproject_core::{ErrorKind, MediaRoot, MediaRootId, SemanticConflictKey, TransactionState};
use postproject_media::{prepare_confirmed_locator, prepare_original_media};
use postproject_storage_sqlite::SqliteProduction;

#[test]
fn retirement_rejects_unbased_work_and_stale_locator_sets_atomically() {
    let directory = tempfile::tempdir().unwrap();
    let media = directory.path().join("original.dat");
    std::fs::write(&media, b"locator decision fixture").unwrap();
    let imported = prepare_original_media(&media, None, None).unwrap();
    let locator = imported.locators()[0].id();
    let resource = imported.resources()[0].id();
    let mut production =
        SqliteProduction::create(directory.path().join("production.pproj"), None).unwrap();
    {
        let mut import = production.begin_transaction().unwrap();
        import.import_original(&imported).unwrap();
        import.commit().unwrap();
    }
    {
        let mut additive = production.begin_transaction().unwrap();
        assert_eq!(
            additive.retire_locator(locator).unwrap_err().kind(),
            ErrorKind::InvalidArgument
        );
        assert_eq!(additive.state(), TransactionState::Open);
        additive
            .add_media_root(
                MediaRoot::new(MediaRootId::new(), "retained", None, None, 0, true).unwrap(),
            )
            .unwrap();
        additive.commit().unwrap();
    }
    assert_eq!(production.locators(resource).unwrap().len(), 1);
    let old_base = production.read_session().unwrap().decision_base();
    {
        let replacement =
            prepare_confirmed_locator(resource, "file:///replacement.dat", None, None).unwrap();
        let mut winner = production.begin_transaction().unwrap();
        winner.add_locator(&replacement).unwrap();
        winner.commit().unwrap();
    }
    let head = production.latest_revision().unwrap().unwrap();
    {
        let mut stale = production.begin_edit(old_base).unwrap();
        stale.retire_locator(locator).unwrap();
        stale
            .add_media_root(
                MediaRoot::new(MediaRootId::new(), "discarded", None, None, 0, true).unwrap(),
            )
            .unwrap();
        let error = stale.commit_with_receipt().unwrap_err();
        assert_eq!(stale.state(), TransactionState::RolledBack);
        let conflict = error.transaction_conflict_detail().unwrap();
        assert_eq!(conflict.key(), &SemanticConflictKey::LocatorSet(resource));
        assert_eq!(conflict.superseding_revision(), head.id());
    }
    assert_eq!(production.locators(resource).unwrap().len(), 2);
    assert_eq!(production.media_roots().unwrap().len(), 1);
    assert_eq!(
        production.latest_revision().unwrap().unwrap().id(),
        head.id()
    );
    let fresh = production.read_session().unwrap().decision_base();
    let mut retirement = production.begin_edit(fresh).unwrap();
    retirement.retire_locator(locator).unwrap();
    retirement.commit().unwrap();
    drop(retirement);
    assert_eq!(production.locators(resource).unwrap().len(), 1);
}
