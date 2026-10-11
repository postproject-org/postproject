//! Typed job helpers shared by hermetic and actual-host qualification.

use std::{sync::Arc, time::Duration};

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, AssetId, ContentStructure,
    Job, JobId, JobKind, Locator, LocatorAvailability, LocatorId, Representation, RepresentationId,
    RepresentationImport, RepresentationKind, RequestedJobOutput, Resource, ResourceId,
    RevisionContext, ToolIdentity,
};
use postproject_protocol::{
    ClientId, Command, Extensions, Outcome, OutcomeStatus, Proposal, RequestId,
};

use super::{Fixture, fixture::Clock};
use crate::SqliteProduction;

struct Scenario<'a> {
    source: &'a mut SqliteProduction,
    clock: Arc<Clock>,
    asset: AssetId,
    input: RepresentationId,
}

impl Scenario<'_> {
    fn proposal(&self, commands: Vec<Command>) -> Proposal {
        Proposal::new(
            self.source.exchange_scope().unwrap(),
            ClientId::new(),
            RequestId::new(),
            None,
            RevisionContext::default(),
            commands,
            Extensions::default(),
        )
        .unwrap()
    }

    fn accepted(&mut self, commands: Vec<Command>, tokens: &[&str]) -> Outcome {
        let proposal = self.proposal(commands);
        let result = self
            .source
            .submit_proposal_with_capabilities(&proposal, &[], tokens)
            .unwrap();
        assert_eq!(result.leases().len(), 0);
        let outcome = result.into_parts().0;
        assert!(
            matches!(outcome.status(), OutcomeStatus::Accepted(_)),
            "{outcome:?}"
        );
        let duplicate = self
            .source
            .submit_proposal_with_capabilities(&proposal, &[], tokens)
            .unwrap();
        assert_eq!(duplicate.outcome(), &outcome);
        assert_eq!(duplicate.leases().len(), 0);
        outcome
    }

    fn request(&mut self, count: usize) -> Vec<JobId> {
        let jobs: Vec<_> = (0..count)
            .map(|_| {
                Job::new(
                    JobId::new(),
                    JobKind::new("vendor:qualification").unwrap(),
                    vec![self.input],
                    RequestedJobOutput::new(self.asset, RepresentationKind::Proxy, None).unwrap(),
                )
                .unwrap()
            })
            .collect();
        // Ordinary native writes must join exactly the same portable history.
        let mut edit = self.source.begin_transaction().unwrap();
        for job in &jobs {
            edit.request_job(job).unwrap();
        }
        edit.commit().unwrap();
        jobs.iter().map(Job::id).collect()
    }

    fn claim(&mut self, job: JobId, duration: u64) -> String {
        let proposal = self.proposal(vec![Command::ClaimJob {
            job_id: job,
            tool: ToolIdentity::new("exact worker 名", Some("1".into()), None).unwrap(),
            agent: None,
            duration: Duration::from_micros(duration),
        }]);
        let (outcome, mut leases) = self
            .source
            .submit_proposal_with_capabilities(&proposal, &[], &[])
            .unwrap()
            .into_parts();
        assert!(
            matches!(outcome.status(), OutcomeStatus::Accepted(_)),
            "{outcome:?}"
        );
        assert_eq!(leases.len(), 1);
        let token = leases.pop().unwrap().export_token().unwrap();
        let duplicate = self
            .source
            .submit_proposal_with_capabilities(&proposal, &[], &[])
            .unwrap();
        assert_eq!(duplicate.outcome(), &outcome);
        assert_eq!(duplicate.leases().len(), 0);
        token
    }

    fn completion(&self, job: JobId) -> Command {
        let resource = ResourceId::new();
        let output = RepresentationImport::new(
            Representation::new(
                RepresentationId::new(),
                self.asset,
                RepresentationKind::Proxy,
                ContentStructure::single_resource(resource),
                vec![],
            ),
            vec![Resource::new(resource, vec![], None)],
            vec![
                Locator::new(
                    LocatorId::new(),
                    resource,
                    "file:///unavailable/worker-output.mov",
                    None,
                    LocatorAvailability::Offline,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let activity = Activity::new(
            ActivityId::new(),
            ActivityKind::new("vendor:qualification").unwrap(),
            vec![ActivityInput::new(self.input, None)],
            vec![ActivityOutput::new(output.representation().id(), None)],
        )
        .unwrap();
        Command::CompleteJob {
            job_id: job,
            output: Box::new(output),
            activity: Box::new(activity),
        }
    }
}

#[test]
fn typed_helpers_join_native_requests_and_recover_completion() {
    let mut fixture = Fixture::new();
    let mut scenario = Scenario {
        asset: fixture.job.requested_output().asset_id(),
        input: fixture.job.inputs()[0],
        source: &mut fixture.source,
        clock: fixture.clock.clone(),
    };
    let jobs = scenario.request(1);
    let token = scenario.claim(jobs[0], 100);
    scenario.clock.set(150);
    scenario.accepted(vec![scenario.completion(jobs[0])], &[&token]);
    assert!(matches!(
        scenario.source.job(jobs[0]).unwrap().state(),
        postproject_core::JobState::Succeeded(_)
    ));
}
