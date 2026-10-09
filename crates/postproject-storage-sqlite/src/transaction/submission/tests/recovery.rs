use std::time::Duration;

use postproject_core::{ErrorKind, JobClaimId, JobLeaseState, JobState, ToolIdentity};
use postproject_protocol::{Command, FailureKind, JobResult, JobResultState, OutcomeStatus};

use super::Fixture;
use crate::{ExchangeError, ReplayLimits, SqliteProduction};

#[test]
fn completed_token_request_recovers_before_expiry_clock_and_ownership_guards() {
    let mut fixture = Fixture::new();
    let claim = fixture.proposal(vec![Command::ClaimJob {
        job_id: fixture.job.id(),
        tool: ToolIdentity::new("worker", None, None).unwrap(),
        agent: None,
        duration: Duration::from_micros(100),
    }]);
    let (claim_outcome, mut leases) = fixture
        .source
        .submit_proposal_with_capabilities(&claim, &[], &[])
        .unwrap()
        .into_parts();
    assert_eq!(leases.len(), 1);
    let lease = leases.pop().unwrap();
    assert!(matches!(
        lease.state().unwrap(),
        JobLeaseState::Active { .. }
    ));
    let token = lease.export_token().unwrap();
    assert_eq!(
        claim_outcome.jobs(),
        &[JobResult::new(
            fixture.job.id(),
            JobResultState::Claimed(postproject_core::Timestamp::from_unix_micros(200))
        )]
    );
    let secret = token.rsplit(':').next().unwrap();
    assert!(!format!("{claim_outcome:?}").contains(secret));
    assert!(
        !std::str::from_utf8(&claim_outcome.document().unwrap().canonical_bytes().unwrap())
            .unwrap()
            .contains(secret)
    );
    // Claim recovery deliberately never reissues privileged delivery.
    let duplicate = fixture
        .source
        .submit_proposal_with_capabilities(&claim, &[], &[])
        .unwrap();
    assert_eq!(duplicate.outcome(), &claim_outcome);
    assert_eq!(duplicate.leases().len(), 0);
    fixture.clock.set(150);
    let completion = fixture.completion();
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&completion, &[], &[&token])
        .unwrap();
    assert_eq!(result.leases().len(), 0);
    let outcome = result.into_parts().0;
    let JobState::Succeeded(completed) = fixture
        .source
        .job(fixture.job.id())
        .unwrap()
        .state()
        .clone()
    else {
        panic!("completion did not publish a terminal state");
    };
    assert_eq!(
        outcome.jobs(),
        &[JobResult::new(
            fixture.job.id(),
            JobResultState::Succeeded(completed)
        )]
    );
    assert!(
        matches!(outcome.status(), OutcomeStatus::Accepted(receipt)
        if receipt.revision().unwrap().sequence() == 3),
        "{outcome:?}"
    );
    fixture.clock.set(200);
    assert_eq!(
        fixture.source.import_job_lease(&token).unwrap_err().kind(),
        ErrorKind::Conflict
    );
    fixture.reopen();
    fixture.clock.disable();
    let duplicate = fixture
        .source
        .submit_proposal_with_capabilities(&completion, &[], &[&token])
        .unwrap();
    assert_eq!(duplicate.outcome(), &outcome);
    assert_eq!(duplicate.leases().len(), 0);
    let changed = format!(
        "ppl1:{}:{}:{}",
        lease.production_id(),
        lease.job_id(),
        JobClaimId::new()
    );
    for tokens in [vec![], vec![changed.as_str()]] {
        assert!(
            matches!(fixture.source.submit_proposal_with_capabilities(&completion, &[], &tokens),
            Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::RequestIdentityMismatch)
        );
    }
    assert_eq!(
        fixture
            .source
            .submission_outcome(
                completion.scope(),
                completion.client(),
                completion.request()
            )
            .unwrap(),
        Some(outcome)
    );
    assert_eq!(fixture.source.changes_since(0, 10).unwrap().len(), 3);
    assert!(matches!(
        fixture.source.job(fixture.job.id()).unwrap().state(),
        JobState::Succeeded(_)
    ));
    assert_replay(&fixture);
}

fn assert_replay(fixture: &Fixture) {
    let directory = tempfile::tempdir().unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        fixture.source.production(),
        fixture.source.exchange_floor().unwrap(),
    )
    .unwrap();
    for sequence in 1..=3 {
        let mut reader = fixture.source.record_reader(sequence).unwrap();
        let manifest = reader.manifest().clone();
        assert!(
            mirror
                .apply_record(
                    &manifest,
                    std::iter::from_fn(|| reader.next_chunk().transpose()),
                    ReplayLimits::default()
                )
                .unwrap()
        );
    }
    assert_eq!(
        mirror.job(fixture.job.id()).unwrap(),
        fixture.source.job(fixture.job.id()).unwrap()
    );
    assert_eq!(
        mirror.activities().unwrap(),
        fixture.source.activities().unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        fixture.source.exchange_head().unwrap()
    );
}
