//! Durable metadata outcomes use native guards and one writer transaction.

use postproject_core::{
    DecisionBase, ErrorKind, MetadataProperty, MetadataValue, ObjectRef, ProductionId, PropertyId,
    RevisionContext, RevisionId, VocabularyId,
};
use postproject_protocol::{
    ClientId, Command, Extensions, FailureKind, HistoryId, OutcomeStatus, Proposal, ProtocolBase,
    RejectionKind, RequestId, Scope,
};
use postproject_storage_sqlite::{ExchangeError, SqliteProduction};
use rusqlite::Connection;

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:submission:test").unwrap(),
        PropertyId::new("value").unwrap(),
    )
}

fn proposal(
    production: &SqliteProduction,
    base: Option<DecisionBase>,
    commands: Vec<Command>,
) -> Proposal {
    let scope = production.exchange_scope().unwrap();
    Proposal::new(
        scope,
        ClientId::new(),
        RequestId::new(),
        base.map(|base| ProtocolBase::new(scope, base).unwrap()),
        RevisionContext::default(),
        commands,
        Extensions::default(),
    )
    .unwrap()
}

fn append(target: ObjectRef, value: i64) -> Command {
    Command::AppendMetadata {
        target,
        property: property(),
        value: MetadataValue::i64(value),
    }
}

#[test]
fn changing_retry_after_reopen_recovers_its_own_receipt_before_stale_guards() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let initial = proposal(&production, None, vec![append(target, 1)]);
    production.submit_proposal(&initial).unwrap();
    let base = production.read_session().unwrap().decision_base();
    let request = proposal(
        &production,
        Some(base),
        vec![Command::ReplaceMetadata {
            target,
            property: property(),
            values: vec![MetadataValue::i64(2)],
        }],
    );
    let original = production.submit_proposal(&request).unwrap();
    assert!(
        matches!(original.status(), OutcomeStatus::Accepted(receipt) if receipt.revision().unwrap().sequence() == 2)
    );
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(3))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    drop(production); // Response-loss recovery from a new authority facade.
    let mut reopened = SqliteProduction::open(&path).unwrap();
    assert_eq!(
        reopened
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap(),
        Some(original.clone())
    );
    assert_eq!(reopened.submit_proposal(&request).unwrap(), original);
    assert_eq!(
        reopened.metadata_values(target, &property()).unwrap(),
        [MetadataValue::i64(2), MetadataValue::i64(3)]
    );
    assert_eq!(reopened.changes_since(0, 10).unwrap().len(), 3);
    let changed = Proposal::new(
        request.scope(),
        request.client(),
        request.request(),
        request.base(),
        request.context().clone(),
        vec![append(target, 99)],
        request.extensions().clone(),
    )
    .unwrap();
    assert!(
        matches!(reopened.submit_proposal(&changed), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::RequestIdentityMismatch)
    );
    assert_eq!(
        reopened
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap(),
        Some(original)
    );
}

#[test]
fn terminal_staging_rejection_discards_earlier_commands_and_survives_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let request = proposal(
        &production,
        None,
        vec![
            append(target, 1),
            Command::RemoveMetadata {
                target,
                property: property(),
            },
        ],
    );
    let outcome = production.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Rejected(rejection) if rejection.kind() == RejectionKind::Domain(ErrorKind::InvalidArgument))
    );
    assert_eq!(
        production.metadata_values(target, &property()).unwrap(),
        Vec::<MetadataValue>::new()
    );
    assert_eq!(
        production.changes_since(0, 10).unwrap(),
        Vec::<postproject_core::Revision>::new()
    );
    drop(production);
    let mut production = SqliteProduction::open(&path).unwrap();
    assert_eq!(production.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(
        production.changes_since(0, 10).unwrap(),
        Vec::<postproject_core::Revision>::new()
    );
}

#[test]
fn accepted_no_op_is_retained_without_a_revision() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let base = production.read_session().unwrap().decision_base();
    let request = proposal(
        &production,
        Some(base),
        vec![Command::ReplaceMetadata {
            target,
            property: property(),
            values: vec![],
        }],
    );
    let outcome = production.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Accepted(receipt) if receipt.revision().is_none())
    );
    assert_eq!(production.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(
        production.changes_since(0, 10).unwrap(),
        Vec::<postproject_core::Revision>::new()
    );
    assert_eq!(
        Connection::open(&path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM exchange_outcomes", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn nonexistent_and_forged_bases_are_terminal_but_other_scopes_are_not_retained() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let base =
        DecisionBase::new(production.production().id(), Some(RevisionId::new()), 17).unwrap();
    let request = proposal(&production, Some(base), vec![append(target, 1)]);
    let outcome = production.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Rejected(rejection) if rejection.kind() == RejectionKind::InvalidBase)
    );
    assert_eq!(production.submit_proposal(&request).unwrap(), outcome);
    for scope in [
        Scope::new(ProductionId::new(), request.scope().history()),
        Scope::new(request.scope().production(), HistoryId::new()),
    ] {
        let foreign = Proposal::new(
            scope,
            request.client(),
            request.request(),
            None,
            RevisionContext::default(),
            vec![],
            Extensions::default(),
        )
        .unwrap();
        assert!(
            matches!(production.submit_proposal(&foreign), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::ScopeMismatch)
        );
    }
    assert_eq!(
        Connection::open(&path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM exchange_outcomes", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn outcome_persistence_failure_rolls_back_domain_and_can_retry_same_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let request = proposal(&production, None, vec![append(target, 1)]);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_outcome BEFORE INSERT ON exchange_outcomes BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    assert!(
        matches!(production.submit_proposal(&request), Err(ExchangeError::Store(error)) if error.kind() == ErrorKind::Storage)
    );
    assert_eq!(
        production.metadata_values(target, &property()).unwrap(),
        Vec::<MetadataValue>::new()
    );
    assert_eq!(
        production.changes_since(0, 10).unwrap(),
        Vec::<postproject_core::Revision>::new()
    );
    assert!(
        production
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap()
            .is_none()
    );
    connection
        .execute_batch("DROP TRIGGER reject_outcome")
        .unwrap();
    assert!(matches!(
        production.submit_proposal(&request).unwrap().status(),
        OutcomeStatus::Accepted(_)
    ));
    assert_eq!(production.changes_since(0, 10).unwrap().len(), 1);
}
