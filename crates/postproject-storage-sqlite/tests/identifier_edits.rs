//! Exact identifier removal must not erase an attachment renewed after a read.

use postproject_core::{
    ErrorKind, ExternalIdentifier, IdentifierScheme, MediaRoot, MediaRootId, ObjectRef,
    SemanticConflictKey, TransactionState,
};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;

#[test]
fn identifier_removal_rejects_unbased_work_and_stale_reattachments() {
    let directory = tempfile::tempdir().unwrap();
    let media = directory.path().join("clip.dat");
    std::fs::write(&media, b"identifier decision fixture").unwrap();
    let imported = prepare_original_media(&media, None, None).unwrap();
    let target = ObjectRef::Asset(imported.asset().id());
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("com.example.clip").unwrap(),
        "observed",
        Some("reel".into()),
    )
    .unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("production.pproj"), None).unwrap();
    {
        let mut transaction = production.begin_transaction().unwrap();
        transaction.import_original(&imported).unwrap();
        transaction
            .add_external_identifier(target, &identifier)
            .unwrap();
        transaction.commit().unwrap();
    }
    {
        let mut unbased = production.begin_transaction().unwrap();
        assert_eq!(
            unbased
                .remove_external_identifier(target, &identifier)
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidArgument
        );
        assert_eq!(unbased.state(), TransactionState::Open);
        unbased
            .add_media_root(
                MediaRoot::new(MediaRootId::new(), "retained", None, None, 0, true).unwrap(),
            )
            .unwrap();
        unbased.commit().unwrap();
    }
    let base = production.read_session().unwrap().decision_base();
    {
        let mut replacement = production.begin_edit(base).unwrap();
        replacement
            .remove_external_identifier(target, &identifier)
            .unwrap();
        replacement
            .add_external_identifier(target, &identifier)
            .unwrap();
        replacement.commit().unwrap();
    }
    let head = production.latest_revision().unwrap().unwrap();
    {
        let mut stale = production.begin_edit(base).unwrap();
        stale
            .remove_external_identifier(target, &identifier)
            .unwrap();
        stale
            .add_media_root(
                MediaRoot::new(MediaRootId::new(), "discarded", None, None, 0, true).unwrap(),
            )
            .unwrap();
        let error = stale.commit_with_receipt().unwrap_err();
        assert_eq!(stale.state(), TransactionState::RolledBack);
        assert_eq!(
            error.transaction_conflict_detail().unwrap().key(),
            &SemanticConflictKey::ExternalIdentifier {
                target,
                identifier: identifier.clone()
            }
        );
    }
    assert_eq!(
        production.latest_revision().unwrap().unwrap().id(),
        head.id()
    );
    assert_eq!(
        production.external_identifiers(target).unwrap(),
        std::slice::from_ref(&identifier)
    );
    assert_eq!(production.production().media_roots().len(), 1);
    let fresh = production.read_session().unwrap().decision_base();
    let mut removal = production.begin_edit(fresh).unwrap();
    removal
        .remove_external_identifier(target, &identifier)
        .unwrap();
    removal.commit().unwrap();
    drop(removal);
    assert!(production.external_identifiers(target).unwrap().is_empty());
}
