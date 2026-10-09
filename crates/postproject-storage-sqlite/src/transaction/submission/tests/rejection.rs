use std::time::Duration;

use postproject_core::{
    ErrorKind, JobLeaseState, JobState, MetadataProperty, ObjectRef, PropertyId, ToolIdentity,
    VocabularyId,
};
use postproject_protocol::{Command, OutcomeStatus, RejectionKind};

use super::Fixture;
use crate::ExchangeError;

fn claim(fixture: &Fixture, worker: &str, micros: u64) -> Command {
    Command::ClaimJob {
        job_id: fixture.job.id(),
        tool: ToolIdentity::new(worker, None, None).unwrap(),
        agent: None,
        duration: Duration::from_micros(micros),
    }
}

#[test]
fn rejected_claim_rolls_back_ownership_and_events_but_retains_observed_authority_time() {
    let mut fixture = Fixture::new();
    let request = fixture.proposal(vec![
        claim(&fixture, "worker", 100),
        Command::RemoveMetadata {
            target: ObjectRef::Job(fixture.job.id()),
            property: MetadataProperty::new(
                VocabularyId::new("urn:test").unwrap(),
                PropertyId::new("missing-base").unwrap(),
            ),
        },
    ]);
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&request, &[], &[])
        .unwrap();
    assert_eq!(result.leases().len(), 0);
    let outcome = result.into_parts().0;
    assert!(
        matches!(outcome.status(), OutcomeStatus::Rejected(rejection)
        if rejection.kind() == RejectionKind::Domain(ErrorKind::InvalidArgument)),
        "{outcome:?}"
    );
    assert_eq!(
        fixture.source.job(fixture.job.id()).unwrap().state(),
        &JobState::Requested
    );
    assert_eq!(fixture.source.changes_since(0, 10).unwrap().len(), 1);
    let retained: Option<i64> = fixture
        .source
        .connection
        .query_row("SELECT high_water_micros FROM job_clock", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(retained, Some(100));
    fixture.clock.set(99);
    let backward = fixture.proposal(vec![claim(&fixture, "backward", 100)]);
    let rejected = fixture.source.submit_proposal(&backward).unwrap();
    assert!(
        matches!(rejected.status(), OutcomeStatus::Rejected(rejection)
        if rejection.kind() == RejectionKind::Domain(ErrorKind::Conflict)),
        "{rejected:?}"
    );
    fixture.clock.set(100);
    let fresh = fixture.proposal(vec![claim(&fixture, "new", 100)]);
    let fresh = fixture
        .source
        .submit_proposal_with_capabilities(&fresh, &[], &[])
        .unwrap();
    assert_eq!(fresh.leases().len(), 1);
    assert!(matches!(
        fresh.leases()[0].state().unwrap(),
        JobLeaseState::Active { .. }
    ));
    fixture.reopen();
    fixture.clock.disable();
    assert_eq!(fixture.source.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(fixture.source.submit_proposal(&backward).unwrap(), rejected);
    assert_eq!(fixture.source.changes_since(0, 10).unwrap().len(), 2);
}

#[test]
fn failed_outcome_persistence_never_delivers_ownership_and_same_identity_can_retry() {
    let mut fixture = Fixture::new();
    let request = fixture.proposal(vec![claim(&fixture, "worker", 100)]);
    fixture
        .source
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_outcome BEFORE INSERT ON exchange_outcomes
        BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .unwrap();
    assert!(
        matches!(fixture.source.submit_proposal_with_capabilities(&request, &[], &[]),
        Err(ExchangeError::Store(error)) if error.kind() == ErrorKind::Storage)
    );
    assert_eq!(
        fixture.source.job(fixture.job.id()).unwrap().state(),
        &JobState::Requested
    );
    assert_eq!(fixture.source.changes_since(0, 10).unwrap().len(), 1);
    assert!(
        fixture
            .source
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap()
            .is_none()
    );
    fixture
        .source
        .connection
        .execute_batch("DROP TRIGGER reject_outcome")
        .unwrap();
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&request, &[], &[])
        .unwrap();
    assert_eq!(result.leases().len(), 1);
    assert!(matches!(
        result.leases()[0].state().unwrap(),
        JobLeaseState::Active { .. }
    ));
    assert_eq!(fixture.source.changes_since(0, 10).unwrap().len(), 2);
}

#[test]
fn only_the_final_active_claim_is_delivered_and_closed_borrowed_handles_recover() {
    let mut fixture = Fixture::new();
    let request = fixture.proposal(vec![
        claim(&fixture, "first", 100),
        Command::ReleaseJob(fixture.job.id()),
        claim(&fixture, "second", 200),
    ]);
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&request, &[], &[])
        .unwrap();
    assert_eq!(result.leases().len(), 1);
    let (_, mut leases) = result.into_parts();
    let lease = leases.pop().unwrap();
    let job = fixture.source.job(fixture.job.id()).unwrap();
    let JobState::Claimed(observed) = job.state() else {
        panic!("not claimed")
    };
    assert_eq!(observed.tool().name(), "second");
    fixture
        .source
        .import_job_lease(&lease.export_token().unwrap())
        .unwrap();
    let release = fixture.proposal(vec![Command::ReleaseJob(fixture.job.id())]);
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&release, &[&lease], &[])
        .unwrap();
    assert_eq!(result.leases().len(), 0);
    assert_eq!(lease.state().unwrap(), JobLeaseState::Closed);
    let outcome = result.into_parts().0;
    fixture.clock.disable();
    assert_eq!(
        fixture
            .source
            .submit_proposal_with_capabilities(&release, &[&lease], &[])
            .unwrap()
            .outcome(),
        &outcome
    );
    assert_eq!(
        fixture.source.job(fixture.job.id()).unwrap().state(),
        &JobState::Requested
    );
    assert_eq!(fixture.source.changes_since(0, 10).unwrap().len(), 3);
}

#[test]
fn administrative_cancellation_never_activates_same_edit_claims() {
    let mut fixture = Fixture::new();
    let request = fixture.proposal(vec![
        claim(&fixture, "worker", 100),
        Command::CancelJob(fixture.job.id()),
    ]);
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&request, &[], &[])
        .unwrap();
    assert_eq!(result.leases().len(), 0);
    assert_eq!(
        fixture.source.job(fixture.job.id()).unwrap().state(),
        &JobState::Cancelled
    );
    assert_eq!(fixture.source.changes_since(0, 10).unwrap().len(), 2);

    // The native API owns the same final delivery rule, not just submission.
    let mut native = Fixture::new();
    let mut edit = native.source.begin_transaction().unwrap();
    let lease = edit
        .claim_job_lease(
            native.job.id(),
            &ToolIdentity::new("worker", None, None).unwrap(),
            None,
            Duration::from_micros(100),
        )
        .unwrap();
    edit.cancel_job(native.job.id()).unwrap();
    assert_eq!(lease.state().unwrap(), JobLeaseState::Pending);
    edit.commit().unwrap();
    assert_eq!(lease.state().unwrap(), JobLeaseState::Closed);
    assert!(lease.export_token().is_err());
}
