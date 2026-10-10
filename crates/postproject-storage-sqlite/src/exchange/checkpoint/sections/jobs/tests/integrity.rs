use super::{JobApply, JobHeader, JobId, JobInput, JobState, RepresentationId, fixture};

#[test]
fn duplicate_headers_missing_inputs_and_noncanonical_positions_reject_and_rollback() {
    let directory = tempfile::tempdir().unwrap();
    let (source, jobs, _) = fixture(&directory.path().join("source.pproj"));
    let head = source.exchange_head().unwrap();
    let job = &jobs[0];
    let header = JobHeader::new(
        JobId::new(),
        job.kind().clone(),
        2,
        job.requested_output().clone(),
        JobState::Requested,
    )
    .unwrap();
    let transaction = source.connection.unchecked_transaction().unwrap();
    let mut pending = JobApply::begin_checkpoint(&transaction, header.clone())
        .unwrap()
        .unwrap();
    assert!(JobApply::begin_checkpoint(&transaction, header.clone()).is_err());
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(header.id(), 1, job.inputs()[0]).unwrap()
            )
            .is_err()
    );
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(header.id(), 0, RepresentationId::new()).unwrap()
            )
            .is_err()
    );
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(JobId::new(), 0, job.inputs()[0]).unwrap()
            )
            .is_err()
    );
    assert!(
        !pending
            .input(
                &transaction,
                JobInput::new(header.id(), 0, job.inputs()[0]).unwrap()
            )
            .unwrap()
    );
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(header.id(), 1, job.inputs()[0]).unwrap()
            )
            .is_err()
    );
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(header.id(), 1, job.inputs()[1]).unwrap()
            )
            .unwrap()
    );
    transaction.rollback().unwrap();
    assert!(source.job(header.id()).is_err());
    assert_eq!(source.exchange_head().unwrap(), head);
}
