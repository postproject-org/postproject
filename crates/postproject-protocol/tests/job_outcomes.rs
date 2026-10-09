//! Outcomes retain canonical final summaries, never whole worker payloads.

use postproject_core::{
    ActivityId, CommitReceipt, JobCompletion, JobId, OriginIdentity, ProductionId,
    RepresentationId, Revision, RevisionContext, RevisionId, Timestamp, TransactionId,
};
use postproject_protocol::{
    ClientId, Command, Document, Extensions, HistoryId, JobResult, JobResultState, Limits, Outcome,
    Proposal, Rejection, RequestId, Scope,
};

fn proposal(jobs: &[JobId], extensions: Extensions) -> Proposal {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    Proposal::new(
        scope,
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        jobs.iter().copied().map(Command::CancelJob).collect(),
        extensions,
    )
    .unwrap()
}

fn decode(value: &serde_json::Value) -> postproject_protocol::Result<Outcome> {
    Outcome::from_document(&Document::parse(
        &serde_json::to_vec(value).unwrap(),
        Outcome::limits(),
    )?)
}

#[test]
fn results_require_exact_sorted_distinct_jobs_and_strict_feature_declarations() {
    let mut ids = [JobId::new(), JobId::new()];
    ids.sort();
    let request = proposal(&[ids[1], ids[0], ids[1]], Extensions::default());
    let receipt = CommitReceipt::new(request.scope().production(), None);
    let jobs: Vec<_> = ids
        .iter()
        .map(|id| JobResult::new(*id, JobResultState::Cancelled))
        .collect();
    assert!(Outcome::accepted(&request, receipt.clone()).is_err());
    for invalid in [
        vec![jobs[0]],
        vec![jobs[1], jobs[0]],
        vec![jobs[0], jobs[0]],
        vec![
            JobResult::new(JobId::new(), JobResultState::Cancelled),
            jobs[1],
        ],
    ] {
        assert!(Outcome::accepted_with_jobs(&request, receipt.clone(), invalid).is_err());
    }
    let outcome = Outcome::accepted_with_jobs(&request, receipt, jobs).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&outcome.document().unwrap().canonical_bytes().unwrap()).unwrap();
    assert_eq!(decode(&value).unwrap(), outcome);
    assert_eq!(
        value["required_features"],
        serde_json::json!(["jobs.v1", "outcomes.v1"])
    );
    for features in [
        serde_json::json!(["outcomes.v1"]),
        serde_json::json!(["outcomes.v1", "jobs.v1"]),
        serde_json::json!(["jobs.v1", "outcomes.v1", "metadata.v1"]),
        serde_json::json!(["future.v1"]),
    ] {
        let mut changed = value.clone();
        changed["required_features"] = features;
        assert!(decode(&changed).is_err());
    }
    let mut rejected: serde_json::Value = serde_json::from_slice(
        &Outcome::rejected(&request, Rejection::invalid_base())
            .unwrap()
            .document()
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    rejected["jobs"] = value["jobs"].clone();
    rejected["required_features"] = value["required_features"].clone();
    assert!(decode(&rejected).is_err());
    let mut duplicated = value;
    duplicated["jobs"][1] = duplicated["jobs"][0].clone();
    assert!(decode(&duplicated).is_err());
}

#[test]
fn maximum_result_collection_extensions_and_escaped_context_fit_advertised_bounds() {
    let mut ids: Vec<_> = (0..1000).map(|_| JobId::new()).collect();
    ids.sort();
    let extensions = Extensions::new(
        Document::parse(
            serde_json::to_string(&serde_json::json!({"vendor:large":"a".repeat(65_500)}))
                .unwrap()
                .as_bytes(),
            Limits::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let request = proposal(&ids, extensions);
    // The largest summary alternative has three UUIDs, independent of the
    // previously claimed attribution or failure diagnostic lengths.
    let jobs = ids
        .into_iter()
        .map(|id| {
            JobResult::new(
                id,
                JobResultState::Succeeded(JobCompletion::new(
                    ActivityId::new(),
                    RepresentationId::new(),
                )),
            )
        })
        .collect();
    let context = RevisionContext::new(
        Some(
            OriginIdentity::new(
                "\u{1}".repeat(256),
                Some("\u{1}".repeat(128)),
                Some(format!("urn:{}", "x".repeat(4092))),
            )
            .unwrap(),
        ),
        Some("\u{1}".repeat(4096)),
    )
    .unwrap();
    let revision = Revision::new(
        RevisionId::new(),
        1,
        TransactionId::new(),
        Timestamp::from_unix_micros(i64::MAX),
        context.origin().cloned(),
        context.message().map(str::to_owned),
    )
    .unwrap();
    let outcome = Outcome::accepted_with_jobs(
        &request,
        CommitReceipt::new(request.scope().production(), Some(revision)),
        jobs,
    )
    .unwrap();
    let bytes = outcome.document().unwrap().canonical_bytes().unwrap();
    assert!(bytes.len() > 128 * 1024);
    assert!(bytes.len() < Outcome::limits().max_bytes());
    assert_eq!(
        Outcome::from_document(&Document::parse(&bytes, Outcome::limits()).unwrap()).unwrap(),
        outcome
    );
}

#[test]
fn retained_development_profile_remains_readable_with_original_request_digest() {
    let request = proposal(&[], Extensions::default());
    let original = Outcome::accepted(
        &request,
        CommitReceipt::new(request.scope().production(), None),
    )
    .unwrap();
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&original.document().unwrap().canonical_bytes().unwrap()).unwrap();
    legacy["required_features"] = serde_json::json!(["metadata.v1"]);
    legacy.as_object_mut().unwrap().remove("jobs");
    assert_eq!(decode(&legacy).unwrap(), original);
    legacy["claim_id"] = serde_json::json!("secret");
    assert!(decode(&legacy).is_err());
}
