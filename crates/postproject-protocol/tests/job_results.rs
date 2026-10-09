//! Result summaries retain exact commit facts without replicating large payloads.

use postproject_core::{
    ActivityId, JobClaim, JobCompletion, JobFailure, JobId, JobState, RepresentationId, Timestamp,
    ToolIdentity,
};
use postproject_protocol::{Document, JobResult, JobResultState, Limits};

#[test]
fn every_lifecycle_summary_preserves_recovery_facts_without_large_attribution() {
    let job = JobId::new();
    let completion = JobCompletion::new(ActivityId::new(), RepresentationId::new());
    for (state, expected) in [
        (JobState::Requested, JobResultState::Requested),
        (
            JobState::Claimed(JobClaim::new(
                ToolIdentity::new("private worker", None, None).unwrap(),
                None,
                Timestamp::from_unix_micros(9_007_199_254_740_993),
            )),
            JobResultState::Claimed(Timestamp::from_unix_micros(9_007_199_254_740_993)),
        ),
        (
            JobState::Succeeded(completion),
            JobResultState::Succeeded(completion),
        ),
        (
            JobState::Failed(JobFailure::new("private failure".repeat(200)).unwrap()),
            JobResultState::Failed,
        ),
        (JobState::Cancelled, JobResultState::Cancelled),
    ] {
        let result = JobResult::from_state(job, &state).unwrap();
        assert_eq!(result.job_id(), job);
        assert_eq!(result.state(), expected);
        let bytes = result.document().canonical_bytes().unwrap();
        assert!(bytes.len() < 256);
        assert!(!String::from_utf8_lossy(&bytes).contains("private"));
        assert_eq!(
            JobResult::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            result
        );
    }
    for expiry in [i64::MIN, i64::MAX] {
        let result = JobResult::new(
            job,
            JobResultState::Claimed(Timestamp::from_unix_micros(expiry)),
        );
        assert_eq!(
            JobResult::from_document(&result.document()).unwrap(),
            result
        );
    }
}

#[test]
fn result_summaries_reject_extra_privilege_fields_and_inexact_observations() {
    let job = JobId::new();
    for state in [
        r#"{"kind":"claimed","expires_at_micros":true}"#,
        r#"{"kind":"claimed","expires_at_micros":"01"}"#,
        r#"{"kind":"claimed","expires_at_micros":"9223372036854775808"}"#,
        r#"{"kind":"claimed","expires_at_micros":"1","claim_id":"secret"}"#,
        r#"{"kind":"failed","diagnostic":"ordinary but outside this summary"}"#,
        r#"{"kind":"succeeded","activity_id":null,"representation_id":null}"#,
        r#"{"kind":"unknown"}"#,
    ] {
        let bytes = format!(r#"{{"job_id":"{job}","state":{state}}}"#);
        let document = Document::parse(bytes.as_bytes(), Limits::default()).unwrap();
        assert!(JobResult::from_document(&document).is_err());
    }
}
