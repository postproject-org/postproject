use super::*;
use postproject_core::ErrorKind;
use postproject_protocol::RejectionKind;

#[test]
fn rejected_media_staging_discards_all_earlier_commands_and_is_terminal() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let root = root();
    let request = proposal(
        &source,
        None,
        vec![
            Command::AddMediaRoot(root),
            Command::RetireLocator(LocatorId::new()),
        ],
    );
    let head = source.exchange_head().unwrap();
    let outcome = source.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Rejected(rejection)
        if rejection.kind() == RejectionKind::Domain(ErrorKind::InvalidArgument))
    );
    assert_eq!(source.media_roots().unwrap(), [] as [MediaRoot; 0]);
    assert_eq!(source.exchange_head().unwrap(), head);
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    assert_eq!(source.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(
        source.changes_since(0, 10).unwrap(),
        [] as [postproject_core::Revision; 0]
    );
}

#[test]
fn unchanged_root_has_no_revision_and_stale_change_conflicts() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let root = root();
    let add = proposal(&source, None, vec![Command::AddMediaRoot(root.clone())]);
    source.submit_proposal(&add).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let unchanged = proposal(
        &source,
        Some(base),
        vec![Command::SetMediaRootEnabled {
            root_id: root.id(),
            enabled: true,
        }],
    );
    let outcome = source.submit_proposal(&unchanged).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Accepted(receipt) if receipt.revision().is_none())
    );
    let disable = proposal(
        &source,
        Some(base),
        vec![Command::SetMediaRootEnabled {
            root_id: root.id(),
            enabled: false,
        }],
    );
    source.submit_proposal(&disable).unwrap();
    let stale = proposal(
        &source,
        Some(base),
        vec![Command::SetMediaRootEnabled {
            root_id: root.id(),
            enabled: true,
        }],
    );
    let rejection = source.submit_proposal(&stale).unwrap();
    assert!(
        matches!(rejection.status(), OutcomeStatus::Rejected(rejection)
        if rejection.kind() == RejectionKind::Domain(ErrorKind::Conflict))
    );
    assert_eq!(source.submit_proposal(&unchanged).unwrap(), outcome);
    assert_eq!(source.submit_proposal(&stale).unwrap(), rejection);
    assert_eq!(source.changes_since(0, 10).unwrap().len(), 2);
    assert!(!source.media_roots().unwrap()[0].is_enabled());
}
