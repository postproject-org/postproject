//! Root decisions reject unbased writes without discarding additive work.

use postproject_core::{ErrorKind, MediaRoot, MediaRootId, SemanticConflictKey, TransactionState};
use postproject_storage_sqlite::SqliteProduction;

#[test]
fn root_edits_require_bases_and_reject_intervening_changes_atomically() {
    let directory = tempfile::tempdir().unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("roots.pproj"), None).unwrap();
    let root_id = MediaRootId::new();
    {
        let mut additive = production.begin_transaction().unwrap();
        additive
            .add_media_root(MediaRoot::new(root_id, "rushes", None, None, 0, true).unwrap())
            .unwrap();
        for enabled in [true, false] {
            assert_eq!(
                additive
                    .set_media_root_enabled(root_id, enabled)
                    .unwrap_err()
                    .kind(),
                ErrorKind::InvalidArgument
            );
        }
        assert_eq!(
            additive.remove_media_root(root_id).unwrap_err().kind(),
            ErrorKind::InvalidArgument
        );
        assert_eq!(additive.state(), TransactionState::Open);
        additive.commit().unwrap();
    }
    assert!(production.media_roots().unwrap()[0].is_enabled());
    let old_base = production.read_session().unwrap().decision_base();
    {
        let mut winner = production.begin_edit(old_base).unwrap();
        winner.set_media_root_enabled(root_id, false).unwrap();
        winner.commit().unwrap();
    }
    let head = production.latest_revision().unwrap().unwrap();
    {
        let mut stale = production.begin_edit(old_base).unwrap();
        stale.remove_media_root(root_id).unwrap();
        let error = stale.commit_with_receipt().unwrap_err();
        assert_eq!(stale.state(), TransactionState::RolledBack);
        let conflict = error.transaction_conflict_detail().unwrap();
        assert_eq!(conflict.key(), &SemanticConflictKey::MediaRoot(root_id));
        assert_eq!(conflict.superseding_revision(), head.id());
    }
    assert_eq!(production.media_roots().unwrap().len(), 1);
    assert!(!production.media_roots().unwrap()[0].is_enabled());
    assert_eq!(
        production.latest_revision().unwrap().unwrap().id(),
        head.id()
    );
    let fresh = production.read_session().unwrap().decision_base();
    let mut removal = production.begin_edit(fresh).unwrap();
    removal.remove_media_root(root_id).unwrap();
    removal.commit().unwrap();
    drop(removal);
    assert_eq!(
        production.media_roots().unwrap(),
        [] as [postproject_core::MediaRoot; 0]
    );
}
