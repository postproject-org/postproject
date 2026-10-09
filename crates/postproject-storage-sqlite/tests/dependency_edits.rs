//! Complete dependency observations reject unbased and stale replacements.

use postproject_core::{
    DecisionBase, Dependency, DependencyKind, DependencyTarget, ErrorKind, MediaRoot, MediaRootId,
    RepresentationId, SemanticConflictKey, TransactionState,
};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;

fn assert_stale_unchanged(
    production: &mut SqliteProduction,
    old: DecisionBase,
    source: RepresentationId,
    dependency: &Dependency,
) {
    let mut stale = production.begin_edit(old).unwrap();
    assert!(
        !stale
            .record_dependency_set(source, std::slice::from_ref(dependency))
            .unwrap()
    );
    let error = stale.commit_with_receipt().unwrap_err();
    assert_eq!(stale.state(), TransactionState::RolledBack);
    assert_eq!(
        error.transaction_conflict_detail().unwrap().key(),
        &SemanticConflictKey::DependencySet(source)
    );
}

#[test]
fn complete_observations_require_bases_and_preserve_a_newer_set() {
    let directory = tempfile::tempdir().unwrap();
    let media = directory.path().join("clip.dat");
    std::fs::write(&media, b"dependency decision fixture").unwrap();
    let imported = prepare_original_media(&media, None, None).unwrap();
    let source = imported.representation().id();
    let dependency = Dependency::new(
        None,
        DependencyKind::new("com.example:reference").unwrap(),
        DependencyTarget::Asset(imported.asset().id()),
        Some(source),
        true,
        "self-reference",
    )
    .unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("production.pproj"), None).unwrap();
    {
        let mut transaction = production.begin_transaction().unwrap();
        transaction.import_original(&imported).unwrap();
        assert_eq!(
            transaction
                .record_dependency_set(source, &[])
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidArgument
        );
        assert_eq!(transaction.state(), TransactionState::Open);
        transaction.commit().unwrap();
    }
    assert!(production.dependency_set(source).unwrap().is_none());
    let initial = production.read_session().unwrap().decision_base();
    {
        let mut edit = production.begin_edit(initial).unwrap();
        assert!(edit.record_dependency_set(source, &[]).unwrap());
        edit.commit().unwrap();
    }
    let old = production.read_session().unwrap().decision_base();
    {
        let mut winner = production.begin_edit(old).unwrap();
        winner
            .record_dependency_set(source, std::slice::from_ref(&dependency))
            .unwrap();
        winner.commit().unwrap();
    }
    let head = production.latest_revision().unwrap().unwrap();
    assert_stale_unchanged(&mut production, old, source, &dependency);
    {
        let mut unbased = production.begin_transaction().unwrap();
        assert_eq!(
            unbased
                .record_dependency_set(source, std::slice::from_ref(&dependency))
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidArgument
        );
        unbased.rollback().unwrap();
    }
    {
        let mut stale = production.begin_edit(old).unwrap();
        stale.record_dependency_set(source, &[]).unwrap();
        stale
            .add_media_root(
                MediaRoot::new(MediaRootId::new(), "discarded", None, None, 0, true).unwrap(),
            )
            .unwrap();
        let error = stale.commit_with_receipt().unwrap_err();
        assert_eq!(stale.state(), TransactionState::RolledBack);
        assert_eq!(
            error.transaction_conflict_detail().unwrap().key(),
            &SemanticConflictKey::DependencySet(source)
        );
    }
    assert_eq!(
        production.latest_revision().unwrap().unwrap().id(),
        head.id()
    );
    assert_eq!(
        production
            .dependency_set(source)
            .unwrap()
            .unwrap()
            .dependencies(),
        std::slice::from_ref(&dependency)
    );
    assert_eq!(production.media_roots().unwrap(), [] as [MediaRoot; 0]);
    let fresh = production.read_session().unwrap().decision_base();
    let mut unchanged = production.begin_edit(fresh).unwrap();
    assert!(
        !unchanged
            .record_dependency_set(source, std::slice::from_ref(&dependency))
            .unwrap()
    );
    assert!(
        unchanged
            .commit_with_receipt()
            .unwrap()
            .revision()
            .is_none()
    );
}

#[test]
fn large_native_dependency_comparison_is_independent_of_public_read_budget() {
    let directory = tempfile::tempdir().unwrap();
    let media = directory.path().join("clip.dat");
    std::fs::write(&media, b"large dependency observation").unwrap();
    let imported = prepare_original_media(&media, None, None).unwrap();
    let source = imported.representation().id();
    let dependency = Dependency::new(
        None,
        DependencyKind::new("example:reference").unwrap(),
        DependencyTarget::Representation(source),
        None,
        true,
        "x".repeat(4096),
    )
    .unwrap();
    let dependencies = vec![dependency; 16_400];
    let path = directory.path().join("large.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).unwrap();
    transaction.import_original(&imported).unwrap();
    assert!(
        transaction
            .record_dependency_set(source, &dependencies)
            .unwrap()
    );
    transaction.commit().unwrap();
    drop(transaction);
    match production.dependency_set(source) {
        Err(error) => assert_eq!(error.kind(), ErrorKind::Unsupported),
        Ok(_) => panic!("public read should exceed 64 MiB"),
    }
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).unwrap();
    assert!(
        !transaction
            .record_dependency_set(source, &dependencies)
            .unwrap()
    );
    assert!(transaction.commit().unwrap().revision().is_none());
    drop(transaction);
    let mut replacement = dependencies;
    replacement.pop();
    let mut transaction = production.begin_edit(base).unwrap();
    assert!(
        transaction
            .record_dependency_set(source, &replacement)
            .unwrap()
    );
    assert!(transaction.commit().unwrap().revision().is_some());
    drop(transaction);
    let connection = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM dependencies", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        16399
    );
}
