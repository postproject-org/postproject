use std::time::Duration;

use postproject_core::{ErrorKind, JobFailure, JobState, ResourceFingerprint};
use postproject_protocol::{Command, OutcomeStatus, Proposal, RejectionKind};

use super::{Fixture, Scenario, secrets::assert_public_secrets_absent};
use crate::SqliteProduction;

fn rejected(
    scenario: &mut Scenario<'_>,
    proposal: &Proposal,
    token: &str,
) -> postproject_protocol::Outcome {
    let result = scenario
        .source
        .submit_proposal_with_capabilities(proposal, &[], &[token])
        .unwrap();
    assert_eq!(result.leases().len(), 0);
    let outcome = result.into_parts().0;
    assert!(
        matches!(outcome.status(), OutcomeStatus::Rejected(value)
        if value.kind() == RejectionKind::Domain(ErrorKind::Conflict)),
        "{outcome:?}"
    );
    assert_eq!(
        scenario
            .source
            .submit_proposal_with_capabilities(proposal, &[], &[token])
            .unwrap()
            .outcome(),
        &outcome
    );
    outcome
}

pub(super) fn run(scenario: &mut Scenario<'_>) {
    let jobs = scenario.request(7);
    let observed = scenario.claim(jobs[1], 10_000);
    scenario.accepted(vec![Command::CancelJob(jobs[2])], &[]);
    let released = scenario.claim(jobs[3], 100);
    scenario.accepted(
        vec![
            Command::RenewJob {
                job_id: jobs[3],
                duration: Duration::from_micros(1000),
            },
            Command::ReleaseJob(jobs[3]),
        ],
        &[&released],
    );
    let failed = scenario.claim(jobs[3], 1000);
    scenario.accepted(
        vec![Command::FailJob {
            job_id: jobs[3],
            failure: JobFailure::new("  Exact failure 名\n  ").unwrap(),
        }],
        &[&failed],
    );
    let completed = scenario.claim(jobs[4], 1000);
    let completion = scenario.proposal(vec![scenario.completion(jobs[4])]);
    let completion_outcome = scenario
        .source
        .submit_proposal_with_capabilities(&completion, &[], &[&completed])
        .unwrap()
        .into_parts()
        .0;
    assert!(
        matches!(completion_outcome.status(), OutcomeStatus::Accepted(_)),
        "{completion_outcome:?}"
    );
    let stale = scenario.claim(jobs[5], 1000);
    let resource = scenario.source.resources(scenario.input).unwrap()[0].id();
    let base = scenario.source.read_session().unwrap().decision_base();
    let mut edit = scenario
        .source
        .begin_transaction_at(base.revision_id().unwrap())
        .unwrap();
    edit.record_resource_fingerprint(
        resource,
        &ResourceFingerprint::new("vendor", 2, vec![0, 255]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let stale_completion = scenario.proposal(vec![scenario.completion(jobs[5])]);
    let rejected_outcome = rejected(scenario, &stale_completion, &stale);
    let expired = scenario.claim(jobs[6], 10);
    scenario.clock.set(500);
    let replacement = scenario.claim(jobs[6], 1000);
    let fenced = scenario.proposal(vec![Command::RenewJob {
        job_id: jobs[6],
        duration: Duration::from_micros(1000),
    }]);
    let fenced_outcome = rejected(scenario, &fenced, &expired);
    scenario.accepted(vec![Command::ReleaseJob(jobs[6])], &[&replacement]);
    assert_states(scenario, &jobs);

    let path = scenario.source.path.clone();
    *scenario.source = SqliteProduction::open(path).unwrap();
    scenario.source.job_clock = scenario.clock.clone();
    scenario.clock.disable();
    for (proposal, token, original) in [
        (&completion, &completed, &completion_outcome),
        (&stale_completion, &stale, &rejected_outcome),
        (&fenced, &expired, &fenced_outcome),
    ] {
        let recovered = scenario
            .source
            .submit_proposal_with_capabilities(proposal, &[], &[token])
            .unwrap();
        assert_eq!(recovered.outcome(), original);
        assert_eq!(recovered.leases().len(), 0);
    }
    assert_public_secrets_absent(
        scenario,
        &[
            observed,
            released,
            failed,
            completed,
            stale,
            expired,
            replacement,
        ],
    );
}

#[test]
fn complete_job_tail_preserves_observations_and_recovery_without_secrets() {
    let mut fixture = Fixture::new();
    run(&mut Scenario {
        asset: fixture.job.requested_output().asset_id(),
        input: fixture.job.inputs()[0],
        source: &mut fixture.source,
        clock: fixture.clock.clone(),
    });
}

fn assert_states(scenario: &Scenario<'_>, jobs: &[postproject_core::JobId]) {
    assert!(matches!(
        scenario.source.job(jobs[0]).unwrap().state(),
        JobState::Requested
    ));
    assert!(matches!(
        scenario.source.job(jobs[1]).unwrap().state(),
        JobState::Claimed(_)
    ));
    assert!(matches!(
        scenario.source.job(jobs[2]).unwrap().state(),
        JobState::Cancelled
    ));
    assert!(matches!(
        scenario.source.job(jobs[3]).unwrap().state(),
        JobState::Failed(_)
    ));
    assert!(matches!(
        scenario.source.job(jobs[4]).unwrap().state(),
        JobState::Succeeded(_)
    ));
    assert!(matches!(
        scenario.source.job(jobs[5]).unwrap().state(),
        JobState::Claimed(_)
    ));
    assert!(matches!(
        scenario.source.job(jobs[6]).unwrap().state(),
        JobState::Requested
    ));
}
