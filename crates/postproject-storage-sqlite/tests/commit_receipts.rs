//! Commit attribution and terminal journal-failure regressions (T06, T07).

use postproject_core::{
    ErrorKind, MediaRoot, MediaRootId, OriginIdentity, RevisionContext, TransactionState,
};
use postproject_storage_sqlite::SqliteProduction;

fn root(name: &str) -> MediaRoot {
    MediaRoot::new(MediaRootId::new(), name, None, None, 0, true).unwrap()
}

#[test]
fn receipt_survives_a_later_writer_and_matches_durable_context() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("receipts.pproj");
    let mut first = SqliteProduction::create(&path, None).unwrap();
    let mut second = SqliteProduction::open(&path).unwrap();
    let receipt = {
        let mut transaction = first.begin_transaction().unwrap();
        transaction
            .set_revision_context(
                RevisionContext::new(
                    Some(OriginIdentity::new("first", None, None).unwrap()),
                    Some("Add rushes".to_owned()),
                )
                .unwrap(),
            )
            .unwrap();
        transaction.add_media_root(root("rushes")).unwrap();
        let receipt = transaction.commit_with_receipt().unwrap();
        assert_eq!(
            receipt.revision().unwrap().transaction_id(),
            transaction.id()
        );
        assert_eq!(transaction.state(), TransactionState::Committed);
        assert_eq!(
            transaction.commit_with_receipt().unwrap_err().kind(),
            ErrorKind::Conflict
        );
        receipt
    };
    {
        let mut transaction = second.begin_transaction().unwrap();
        transaction.add_media_root(root("renders")).unwrap();
        transaction.commit_with_receipt().unwrap();
    }
    assert_eq!(receipt.production_id(), first.production().id());
    assert_eq!(receipt.revision().unwrap().sequence(), 1);
    assert_eq!(first.latest_revision().unwrap().unwrap().sequence(), 2);
    let revisions = first.changes_since(0, 10).unwrap();
    assert_eq!(receipt.revision().unwrap(), &revisions[0]);
    assert_eq!(revisions[0].message(), Some("Add rushes"));
    assert_eq!(revisions[0].origin().unwrap().name(), "first");
}

#[test]
fn no_change_never_claims_the_existing_head() {
    let directory = tempfile::tempdir().unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("noop.pproj"), None).unwrap();
    let id = production.production().id();
    for has_head in [false, true] {
        if has_head {
            let mut transaction = production.begin_transaction().unwrap();
            transaction.add_media_root(root("media")).unwrap();
            transaction.commit().unwrap();
        }
        let head = production.latest_revision().unwrap();
        {
            let mut transaction = production.begin_transaction().unwrap();
            let receipt = transaction.commit_with_receipt().unwrap();
            assert_eq!(receipt.production_id(), id);
            assert!(receipt.revision().is_none());
        }
        assert_eq!(production.latest_revision().unwrap(), head);
    }
}

#[test]
fn journal_preparation_failure_is_terminal_and_rolls_back_facts() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("failure.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_revision BEFORE INSERT ON revisions
             BEGIN SELECT RAISE(ABORT, 'journal rejected'); END;",
        )
        .unwrap();
    {
        let mut transaction = production.begin_transaction().unwrap();
        transaction.add_media_root(root("rejected")).unwrap();
        assert!(transaction.commit_with_receipt().is_err());
        assert_eq!(transaction.state(), TransactionState::RolledBack);
        assert_eq!(
            transaction.rollback().unwrap_err().kind(),
            ErrorKind::Conflict
        );
    }
    assert!(production.latest_revision().unwrap().is_none());
    assert_eq!(
        production.media_roots().unwrap(),
        [] as [postproject_core::MediaRoot; 0]
    );
    assert_eq!(
        SqliteProduction::open(&path)
            .unwrap()
            .media_roots()
            .unwrap(),
        [] as [postproject_core::MediaRoot; 0]
    );
    connection
        .execute_batch("DROP TRIGGER reject_revision")
        .unwrap();
    let mut next = production.begin_transaction().unwrap();
    next.add_media_root(root("accepted")).unwrap();
    assert_eq!(
        next.commit_with_receipt()
            .unwrap()
            .revision()
            .unwrap()
            .sequence(),
        1
    );
}
