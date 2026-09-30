//! Runs the Rust listing in the semantic-conflicts guide.

use postproject_core::{ConflictKeyKind, MediaRoot, MediaRootId, Result, SemanticConflictKey};
use postproject_storage_sqlite::SqliteProduction;

// [semantic-conflicts]
fn update_from_a_base_revision(production: &mut SqliteProduction) -> Result<()> {
    let root_id = MediaRootId::new();
    let root = MediaRoot::new(root_id, "rushes", None, None, 0, true)?;
    {
        let mut setup = production.begin_transaction()?;
        setup.add_media_root(root)?;
        setup.commit()?;
    }

    let base = production.latest_revision()?.expect("setup revision");
    {
        let mut first_writer = production.begin_transaction_at(base.id())?;
        first_writer.set_media_root_enabled(root_id, false)?;
        first_writer.commit()?;
    }

    let superseding = production.latest_revision()?.expect("first update");
    {
        let mut stale_writer = production.begin_transaction_at(base.id())?;
        stale_writer.set_media_root_enabled(root_id, true)?;
        let error = stale_writer.commit().expect_err("stale write conflicts");
        let conflict = error
            .transaction_conflict_detail()
            .expect("structured conflict detail");
        assert_eq!(conflict.key().kind(), ConflictKeyKind::MediaRoot);
        assert_eq!(conflict.key(), &SemanticConflictKey::MediaRoot(root_id));
        assert_eq!(conflict.base_revision(), base.id());
        assert_eq!(conflict.superseding_revision(), superseding.id());
    }

    // Retry only after re-reading and deciding that enabling is still right.
    let refreshed = production.latest_revision()?.expect("refreshed revision");
    {
        let mut retry = production.begin_transaction_at(refreshed.id())?;
        retry.set_media_root_enabled(root_id, true)?;
        retry.commit()
    }
}
// [/semantic-conflicts]

#[test]
fn semantic_conflict_example_runs() -> Result<()> {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("conflicts.pproj"), None)?;
    update_from_a_base_revision(&mut production)
}
