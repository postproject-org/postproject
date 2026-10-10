//! State-specific recovery facts never supply worker ownership.

use std::ptr;

use postproject_core::{JobCompletion, JobId, RevisionContext, Timestamp};
use postproject_protocol::{
    Command, Extensions, HistoryId, JobResult, JobResultState, Outcome, Proposal,
};

use super::{ClientId, PpOutcome, RequestId, Scope, outcome_jobs::*};
use crate::{PpJobCompletion, PpJobId};

fn fixture() -> (PpOutcome, Vec<JobResult>, JobCompletion) {
    let scope = Scope::new(crate::ProductionId::new(), HistoryId::new());
    let completion = JobCompletion::new(crate::ActivityId::new(), crate::RepresentationId::new());
    let states = [
        JobResultState::Requested,
        JobResultState::Claimed(Timestamp::from_unix_micros(-7)),
        JobResultState::Succeeded(completion),
        JobResultState::Failed,
        JobResultState::Cancelled,
    ];
    let jobs: Vec<_> = states
        .into_iter()
        .enumerate()
        .map(|(index, state)| {
            JobResult::new(
                JobId::from_bytes([u8::try_from(index + 1).unwrap(); 16]),
                state,
            )
        })
        .collect();
    let proposal = Proposal::new(
        scope,
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        jobs.iter()
            .map(|job| Command::CancelJob(job.job_id()))
            .collect(),
        Extensions::default(),
    )
    .unwrap();
    let outcome = PpOutcome::new(
        Outcome::accepted_with_jobs(
            &proposal,
            crate::CommitReceipt::new(scope.production(), None),
            jobs.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    (outcome, jobs, completion)
}

#[test]
fn original_job_alternatives_are_checked_and_wrong_payloads_cleared() {
    let (outcome, jobs, completion) = fixture();
    // SAFETY: Fixture owns immutable outcome and writable outputs throughout.
    unsafe {
        assert_eq!(pp_outcome_job_count(&raw const outcome), 5);
        assert_eq!(pp_outcome_job_count(ptr::null()), 0);
        for (index, expected) in jobs.iter().enumerate() {
            let index = u64::try_from(index).unwrap();
            let mut id = PpJobId { bytes: [0; 16] };
            let mut state = 0;
            assert_eq!(
                pp_outcome_job_get(
                    &raw const outcome,
                    index,
                    &raw mut id,
                    &raw mut state,
                    ptr::null_mut()
                ),
                crate::PP_OK
            );
            assert_eq!(id.bytes, *expected.job_id().as_bytes());
            assert_eq!(state, u32::try_from(index + 1).unwrap());
            let mut expiry = 99;
            let mut published = PpJobCompletion::empty();
            published.activity_id.bytes = [1; 16];
            let expiry_status =
                pp_outcome_job_expiry(&raw const outcome, index, &raw mut expiry, ptr::null_mut());
            let completion_status = pp_outcome_job_completion(
                &raw const outcome,
                index,
                &raw mut published,
                ptr::null_mut(),
            );
            if index == 1 {
                assert_eq!(expiry_status, crate::PP_OK);
                assert_eq!(expiry, -7);
            } else {
                assert_eq!(expiry_status, crate::PP_ERROR_INVALID_ARGUMENT);
                assert_eq!(expiry, 0);
            }
            if index == 2 {
                assert_eq!(completion_status, crate::PP_OK);
                assert_eq!(
                    published.activity_id.bytes,
                    *completion.activity_id().as_bytes()
                );
                assert_eq!(
                    published.representation_id.bytes,
                    *completion.representation_id().as_bytes()
                );
            } else {
                assert_eq!(completion_status, crate::PP_ERROR_INVALID_ARGUMENT);
                assert_eq!(published.activity_id.bytes, [0; 16]);
                assert_eq!(published.representation_id.bytes, [0; 16]);
            }
        }
        let mut id = PpJobId { bytes: [1; 16] };
        let mut state = 99;
        assert_eq!(
            pp_outcome_job_get(
                &raw const outcome,
                u64::MAX,
                &raw mut id,
                &raw mut state,
                ptr::null_mut()
            ),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(id.bytes, [0; 16]);
        assert_eq!(state, 0);
    }
}
