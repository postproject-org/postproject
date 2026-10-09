use super::*;
use postproject_core::{CommitReceipt, RevisionContext};
use postproject_protocol::{Extensions, Proposal};

fn request(source: &SqliteProduction) -> Proposal {
    Proposal::new(
        source.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![],
        Extensions::default(),
    )
    .unwrap()
}

#[test]
fn recovery_compares_both_bindings_and_public_lookup_never_requires_a_credential() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let source = SqliteProduction::create(&path, None).unwrap();
    let proposal = request(&source);
    let outcome = Outcome::accepted(
        &proposal,
        CommitReceipt::new(source.production().id(), None),
    )
    .unwrap();
    let binding = [7; 32];
    persist(&source.connection, &outcome, Some(&binding)).unwrap();
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    assert_eq!(
        source
            .submission_outcome(proposal.scope(), proposal.client(), proposal.request())
            .unwrap(),
        Some(outcome.clone())
    );
    assert_eq!(
        lookup_for_submission(
            &source.connection,
            proposal.scope(),
            proposal.client(),
            proposal.request(),
            proposal.digest().unwrap(),
            Some(&binding)
        )
        .unwrap(),
        Some(outcome.clone())
    );
    for (digest, binding) in [
        (proposal.digest().unwrap(), None),
        (proposal.digest().unwrap(), Some([8; 32])),
        (Digest::from_bytes([9; 32]), Some(binding)),
    ] {
        let error = lookup_for_submission(
            &source.connection,
            proposal.scope(),
            proposal.client(),
            proposal.request(),
            digest,
            binding.as_ref(),
        )
        .unwrap_err();
        assert!(
            matches!(error, super::super::error::ExchangeError::Protocol(error)
            if error.kind() == FailureKind::RequestIdentityMismatch)
        );
    }
    // A retry through the non-capability facade must not erase private context.
    assert!(
        matches!(source.submit_proposal(&proposal), Err(super::super::error::ExchangeError::Protocol(error))
        if error.kind() == FailureKind::RequestIdentityMismatch)
    );
    assert_eq!(
        source
            .submission_outcome(proposal.scope(), proposal.client(), proposal.request())
            .unwrap(),
        Some(outcome)
    );
    assert_eq!(
        source.changes_since(0, 10).unwrap(),
        [] as [postproject_core::Revision; 0]
    );
}

#[test]
fn malformed_stored_binding_is_corruption_even_for_public_recovery() {
    let directory = tempfile::tempdir().unwrap();
    let source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let proposal = request(&source);
    let outcome = Outcome::accepted(
        &proposal,
        CommitReceipt::new(source.production().id(), None),
    )
    .unwrap();
    persist(&source.connection, &outcome, None).unwrap();
    source
        .connection
        .execute_batch(
            "PRAGMA ignore_check_constraints = ON;
        UPDATE exchange_outcomes SET capability_binding = zeroblob(31);",
        )
        .unwrap();
    assert!(
        matches!(source.submission_outcome(proposal.scope(), proposal.client(), proposal.request()),
        Err(super::super::error::ExchangeError::Store(error)) if error.kind() == ErrorKind::Storage)
    );
}

#[test]
fn large_job_results_recover_after_reopen_with_their_original_public_facts() {
    use postproject_core::{ActivityId, JobCompletion, JobId, RepresentationId};
    use postproject_protocol::{Command, JobResult, JobResultState};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let source = SqliteProduction::create(&path, None).unwrap();
    let mut ids: Vec<_> = (0..1000).map(|_| JobId::new()).collect();
    ids.sort();
    let proposal = Proposal::new(
        source.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        ids.iter().copied().map(Command::CancelJob).collect(),
        Extensions::default(),
    )
    .unwrap();
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
    let outcome = Outcome::accepted_with_jobs(
        &proposal,
        CommitReceipt::new(source.production().id(), None),
        jobs,
    )
    .unwrap();
    assert!(outcome.document().unwrap().canonical_bytes().unwrap().len() > 128 * 1024);
    persist(&source.connection, &outcome, Some(&[7; 32])).unwrap();
    drop(source);
    let source = SqliteProduction::open(path).unwrap();
    assert_eq!(
        source
            .submission_outcome(proposal.scope(), proposal.client(), proposal.request())
            .unwrap(),
        Some(outcome)
    );
}
