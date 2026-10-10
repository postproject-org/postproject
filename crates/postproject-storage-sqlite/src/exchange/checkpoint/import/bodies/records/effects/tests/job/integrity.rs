use postproject_core::{JobId, JobKind, JobState, ObjectRef};
use postproject_protocol::{
    FrameDecoder, JobHeader, JobInput, JobOperation, JobTransition, Limits,
};
use rusqlite::params;

use crate::{
    ExchangeError, SqliteProduction,
    exchange::{
        checkpoint::import::{activity_state, job_state, media_state, root_state},
        job_capture,
    },
};

fn initialize(connection: &rusqlite::Connection) {
    media_state::create(connection).unwrap();
    root_state::create(connection).unwrap();
    job_state::create(connection).unwrap();
}

#[test]
fn immutable_requests_inputs_creation_order_and_duplicate_authorship_reject() {
    let directory = tempfile::tempdir().unwrap();
    let (source, jobs, _) = crate::exchange::checkpoint::sections::jobs::tests::fixture(
        &directory.path().join("source.pproj"),
    );
    let job = jobs[0].id();
    let transaction = source.connection.unchecked_transaction().unwrap();
    initialize(&transaction);
    assert!(
        job_state::require(&transaction, ObjectRef::Job(job), 1).is_err(),
        "future request is not baseline"
    );
    let current = job_capture::header(&transaction, job).unwrap();
    let request = |kind, count| {
        JobHeader::new(
            job,
            kind,
            count,
            current.requested_output().clone(),
            JobState::Requested,
        )
        .unwrap()
    };
    assert!(
        job_state::Request::begin(
            &transaction,
            request(JobKind::new("unknown:Other").unwrap(), 2),
            1
        )
        .is_err()
    );
    assert!(
        job_state::Request::begin(&transaction, request(current.kind().clone(), 1), 1).is_err()
    );
    let header = request(current.kind().clone(), 2);
    let mut pending = job_state::Request::begin(&transaction, header.clone(), 1).unwrap();
    assert!(job_state::Request::begin(&transaction, header, 1).is_err());
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(job, 1, jobs[0].inputs()[0]).unwrap()
            )
            .is_err()
    );
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(JobId::new(), 0, jobs[0].inputs()[0]).unwrap()
            )
            .is_err()
    );
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(job, 0, jobs[0].inputs()[1]).unwrap()
            )
            .is_err()
    );
    assert!(
        !pending
            .input(
                &transaction,
                JobInput::new(job, 0, jobs[0].inputs()[0]).unwrap()
            )
            .unwrap()
    );
    assert!(
        pending
            .input(
                &transaction,
                JobInput::new(job, 1, jobs[0].inputs()[1]).unwrap()
            )
            .unwrap()
    );
    transaction.rollback().unwrap();
}

fn completion(source: &SqliteProduction, job: JobId) -> JobTransition {
    let mut reader = source.record_reader(6).unwrap();
    let mut decoder = FrameDecoder::new(Limits::default());
    while let Some(chunk) = reader.next_chunk().unwrap() {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            if let Some(document) = document {
                if document.kind().unwrap() == "job.transition" {
                    let change = JobTransition::from_document(&document).unwrap();
                    if change.job_id() == job && change.operation() == JobOperation::Complete {
                        return change;
                    }
                }
            }
        }
    }
    panic!("missing native completion");
}

#[test]
fn completion_requires_original_claim_boundary_and_the_exact_publication_graph() {
    let directory = tempfile::tempdir().unwrap();
    let (source, jobs) = super::publication_fixture(&directory.path().join("source.pproj"));
    let original = completion(&source, jobs[0].id());
    assert_eq!(original.input_boundary(), Some(5));
    let JobState::Succeeded(output) = original.state() else {
        panic!("missing completion");
    };
    let revision = source.latest_revision().unwrap().unwrap().id();
    for alter_inputs in [false, true] {
        let transaction = source.connection.unchecked_transaction().unwrap();
        initialize(&transaction);
        activity_state::create(&transaction, 10_000_000, || Ok(())).unwrap();
        activity_state::created(&transaction, output.activity_id()).unwrap();
        transaction
            .execute(
                "INSERT INTO checkpoint_media_created (kind, id) VALUES (2, ?1)",
                [output.representation_id().as_bytes().as_slice()],
            )
            .unwrap();
        let forged = if alter_inputs {
            transaction
                .execute(
                    "DELETE FROM activity_inputs WHERE activity_id = ?1 AND representation_id = ?2",
                    params![
                        output.activity_id().as_bytes().as_slice(),
                        jobs[0].inputs()[0].as_bytes().as_slice()
                    ],
                )
                .unwrap();
            original.clone()
        } else {
            JobTransition::new(
                original.job_id(),
                original.operation(),
                original.previous().clone(),
                original.state().clone(),
                original.authority_time(),
                Some(6),
            )
            .unwrap()
        };
        let error = job_state::changed(&transaction, &forged, revision, 6, 5).unwrap_err();
        assert!(
            matches!(error, ExchangeError::Protocol(error) if error.kind() == postproject_protocol::FailureKind::Integrity)
        );
        if !alter_inputs {
            job_state::changed(&transaction, &original, revision, 6, 5).unwrap();
        }
        transaction.rollback().unwrap();
    }
    assert_eq!(source.activities().unwrap().len(), 1);
}
