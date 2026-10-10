//! Current requests are immutable; lifecycle evidence follows authored order.

mod lifecycle;
mod request;

pub(super) use lifecycle::changed;
pub(super) use request::{Request, require};

use postproject_core::{JobId, JobState, RevisionEventType};
use postproject_protocol::{Document, JobHeader, Limits};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, exchange::job_capture, sqlite_error};

use super::super::invalid;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_job_state (job BLOB PRIMARY KEY NOT NULL, document BLOB NOT NULL, created INTEGER NOT NULL, claim_sequence INTEGER);")
        .map_err(sqlite_error("create private job expectations"))?;
    Ok(())
}

fn expected(connection: &Connection, job: JobId) -> ExchangeResult<Option<JobHeader>> {
    let bytes: Option<Vec<u8>> = connection
        .query_row(
            "SELECT document FROM checkpoint_job_state WHERE job = ?1",
            [job.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error("read authored job expectation"))?;
    bytes
        .map(|bytes| {
            Ok(JobHeader::from_document(&Document::parse(
                &bytes,
                Limits::default(),
            )?)?)
        })
        .transpose()
}

fn with_state(header: &JobHeader, state: JobState) -> ExchangeResult<JobHeader> {
    Ok(JobHeader::new(
        header.id(),
        header.kind().clone(),
        header.input_count(),
        header.requested_output().clone(),
        state,
    )?)
}

fn store(
    connection: &Connection,
    header: &JobHeader,
    created: bool,
    claim: Option<u64>,
) -> ExchangeResult<()> {
    connection.execute("INSERT INTO checkpoint_job_state (job, document, created, claim_sequence) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(job) DO UPDATE SET document = excluded.document, created = MAX(created, excluded.created), claim_sequence = excluded.claim_sequence", params![header.id().as_bytes().as_slice(), header.document()?.canonical_bytes()?, created, claim.map(i64::try_from).transpose().map_err(|_| invalid())?])
        .map_err(sqlite_error("stage authored job expectation"))?;
    Ok(())
}

fn observed_claim(
    connection: &Connection,
    job: JobId,
    through: u64,
) -> ExchangeResult<Option<u64>> {
    let sequence: Option<i64> = connection.query_row("SELECT MAX(r.sequence) FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind = ?1 AND e.primary_id = ?2 AND r.sequence <= ?3", params![crate::stored_revision_event_kind(RevisionEventType::JobClaimed)?, job.as_bytes().as_slice(), i64::try_from(through).map_err(|_| invalid())?], |row| row.get(0))
        .map_err(sqlite_error("read original claim observation boundary"))?;
    sequence
        .map(|sequence| {
            crate::stored_u64(sequence, "original claim observation").map_err(Into::into)
        })
        .transpose()
}

pub(super) fn finish(connection: &Connection, floor: u64) -> ExchangeResult<()> {
    let mut statement = connection
        .prepare("SELECT id FROM jobs ORDER BY id")
        .map_err(sqlite_error("prepare final job lifecycle audit"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query final job lifecycle audit"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read final job audit identity"))?
    {
        let job = JobId::from_bytes(crate::id_bytes(
            row.get(0)
                .map_err(sqlite_error("read final job identity"))?,
            "job identity",
        )?);
        let current = job_capture::header(connection, job)?;
        match expected(connection, job)? {
            Some(expected) if expected != current => return Err(invalid().into()),
            None => {
                let unexplained: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.primary_id = ?1 AND e.kind IN (?2, ?3, ?4, ?5, ?6, ?7, ?8) AND r.sequence > ?9)", params![job.as_bytes().as_slice(), crate::stored_revision_event_kind(RevisionEventType::JobRequested)?, crate::stored_revision_event_kind(RevisionEventType::JobClaimed)?, crate::stored_revision_event_kind(RevisionEventType::JobClaimRenewed)?, crate::stored_revision_event_kind(RevisionEventType::JobClaimReleased)?, crate::stored_revision_event_kind(RevisionEventType::JobSucceeded)?, crate::stored_revision_event_kind(RevisionEventType::JobFailed)?, crate::stored_revision_event_kind(RevisionEventType::JobCancelled)?, i64::try_from(floor).map_err(|_| invalid())?], |row| row.get(0))
                    .map_err(sqlite_error("check unexplained retained job lifecycle"))?;
                if floor == 0 || unexplained {
                    return Err(invalid().into());
                }
            }
            Some(_) => {}
        }
    }
    connection
        .execute_batch("DROP TABLE checkpoint_job_state;")
        .map_err(sqlite_error("discard private job expectations"))?;
    Ok(())
}
