//! Historical decisions are checked without credentials or a receiver clock.

use postproject_core::{
    ActivityId, JobClaim, JobCompletion, JobFailure, JobId, JobState, RepresentationId, Timestamp,
    ToolIdentity,
};
use postproject_protocol::{JobOperation, JobTransition};

fn claim(expiry: i64, tool: &str) -> JobState {
    JobState::Claimed(JobClaim::new(
        ToolIdentity::new(tool, None, None).unwrap(),
        None,
        Timestamp::from_unix_micros(expiry),
    ))
}

fn completed() -> JobState {
    JobState::Succeeded(JobCompletion::new(
        ActivityId::new(),
        RepresentationId::new(),
    ))
}

#[test]
fn all_native_transitions_preserve_their_exact_original_decisions() {
    let cases = [
        (
            JobOperation::Claim,
            JobState::Requested,
            claim(20, "first"),
            Some(10),
            Some(0),
        ),
        (
            JobOperation::Claim,
            claim(10, "first"),
            claim(30, "second"),
            Some(10),
            Some(7),
        ),
        (
            JobOperation::Renew,
            claim(20, "first"),
            claim(30, "first"),
            Some(10),
            None,
        ),
        (
            JobOperation::Release,
            claim(20, "first"),
            JobState::Requested,
            Some(10),
            None,
        ),
        (
            JobOperation::Fail,
            claim(20, "first"),
            JobState::Failed(JobFailure::new("Exact").unwrap()),
            Some(10),
            None,
        ),
        (
            JobOperation::Complete,
            claim(20, "first"),
            completed(),
            Some(10),
            Some(7),
        ),
        (
            JobOperation::Cancel,
            claim(20, "first"),
            JobState::Cancelled,
            None,
            None,
        ),
        (
            JobOperation::Cancel,
            JobState::Requested,
            JobState::Cancelled,
            None,
            None,
        ),
        (
            JobOperation::Claim,
            JobState::Requested,
            claim(i64::MAX, "first"),
            Some(i64::MAX - 10),
            Some(i64::MAX as u64),
        ),
    ];
    for (operation, previous, state, time, boundary) in cases {
        let change = JobTransition::new(
            JobId::new(),
            operation,
            previous,
            state,
            time.map(Timestamp::from_unix_micros),
            boundary,
        )
        .unwrap();
        assert_eq!(
            JobTransition::from_document(&change.document().unwrap()).unwrap(),
            change
        );
        assert_eq!(change.input_boundary(), boundary);
        assert_eq!(change.authority_time().map(Timestamp::as_unix_micros), time);
    }
}

#[test]
fn recorded_claim_and_renewal_durations_retain_the_native_one_day_limit() {
    let day = 86_400_000_000;
    for operation in [JobOperation::Claim, JobOperation::Renew] {
        let previous = if operation == JobOperation::Claim {
            JobState::Requested
        } else {
            claim(20, "first")
        };
        let boundary = (operation == JobOperation::Claim).then_some(0);
        assert!(
            JobTransition::new(
                JobId::new(),
                operation,
                previous.clone(),
                claim(day + 10, "first"),
                Some(Timestamp::from_unix_micros(10)),
                boundary
            )
            .is_ok()
        );
        for (now, expiry) in [(10, day + 11), (i64::MIN, i64::MAX), (10, 10), (10, 9)] {
            assert!(
                JobTransition::new(
                    JobId::new(),
                    operation,
                    previous.clone(),
                    claim(expiry, "first"),
                    Some(Timestamp::from_unix_micros(now)),
                    boundary
                )
                .is_err()
            );
        }
    }
}

#[test]
fn impossible_or_misattributed_transitions_reject() {
    let cases = [
        (
            JobOperation::Claim,
            claim(20, "first"),
            claim(30, "second"),
            Some(10),
            Some(0),
        ),
        (
            JobOperation::Claim,
            JobState::Requested,
            claim(10, "first"),
            Some(10),
            Some(0),
        ),
        (
            JobOperation::Claim,
            JobState::Requested,
            claim(20, "first"),
            Some(10),
            None,
        ),
        (
            JobOperation::Renew,
            claim(20, "first"),
            claim(30, "second"),
            Some(10),
            None,
        ),
        (
            JobOperation::Renew,
            claim(20, "first"),
            claim(20, "first"),
            Some(10),
            None,
        ),
        (
            JobOperation::Renew,
            claim(10, "first"),
            claim(30, "first"),
            Some(10),
            None,
        ),
        (
            JobOperation::Release,
            claim(10, "first"),
            JobState::Requested,
            Some(10),
            None,
        ),
        (
            JobOperation::Fail,
            JobState::Requested,
            JobState::Failed(JobFailure::new("Exact").unwrap()),
            Some(10),
            None,
        ),
        (
            JobOperation::Complete,
            claim(20, "first"),
            completed(),
            Some(10),
            None,
        ),
        (
            JobOperation::Cancel,
            completed(),
            JobState::Cancelled,
            None,
            None,
        ),
        (
            JobOperation::Cancel,
            JobState::Requested,
            JobState::Cancelled,
            Some(10),
            None,
        ),
        (
            JobOperation::Cancel,
            JobState::Requested,
            JobState::Cancelled,
            None,
            Some(0),
        ),
    ];
    for (operation, previous, state, time, boundary) in cases {
        assert!(
            JobTransition::new(
                JobId::new(),
                operation,
                previous,
                state,
                time.map(Timestamp::from_unix_micros),
                boundary
            )
            .is_err()
        );
    }
}
