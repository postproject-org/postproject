use std::time::Duration;

use postproject_core::{JobFailure, JobState, ToolIdentity};
use postproject_protocol::{Command, OutcomeStatus};

use super::Fixture;
use crate::{ReplayLimits, SqliteProduction};

fn claim(fixture: &Fixture) -> Command {
    Command::ClaimJob {
        job_id: fixture.job.id(),
        tool: ToolIdentity::new("worker", None, None).unwrap(),
        agent: None,
        duration: Duration::from_micros(100),
    }
}

fn replay(fixture: &Fixture) {
    let directory = tempfile::tempdir().unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        fixture.source.production(),
        fixture.source.exchange_floor().unwrap(),
    )
    .unwrap();
    for sequence in 1..=2 {
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

#[test]
fn same_proposal_claim_renew_and_completion_publish_one_revision_and_no_live_capability() {
    let mut fixture = Fixture::new();
    let request = fixture.proposal(vec![
        claim(&fixture),
        Command::RenewJob {
            job_id: fixture.job.id(),
            duration: Duration::from_micros(200),
        },
        Command::CompleteJob {
            job_id: fixture.job.id(),
            output: Box::new(fixture.output.clone()),
            activity: Box::new(fixture.activity.clone()),
        },
    ]);
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&request, &[], &[])
        .unwrap();
    assert_eq!(result.leases().len(), 0);
    assert!(
        matches!(result.outcome().status(), OutcomeStatus::Accepted(receipt)
        if receipt.revision().unwrap().sequence() == 2),
        "{result:?}"
    );
    assert!(matches!(
        fixture.source.job(fixture.job.id()).unwrap().state(),
        JobState::Succeeded(_)
    ));
    replay(&fixture);
}

#[test]
fn same_proposal_claim_renew_and_failure_preserve_recorded_state_without_ownership() {
    let mut fixture = Fixture::new();
    let failure = JobFailure::new("  Exact failure 名  ").unwrap();
    let request = fixture.proposal(vec![
        claim(&fixture),
        Command::RenewJob {
            job_id: fixture.job.id(),
            duration: Duration::from_micros(200),
        },
        Command::FailJob {
            job_id: fixture.job.id(),
            failure: failure.clone(),
        },
    ]);
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&request, &[], &[])
        .unwrap();
    assert_eq!(result.leases().len(), 0);
    assert_eq!(
        fixture.source.job(fixture.job.id()).unwrap().state(),
        &JobState::Failed(failure)
    );
    assert_eq!(fixture.source.changes_since(0, 10).unwrap().len(), 2);
    replay(&fixture);
}
