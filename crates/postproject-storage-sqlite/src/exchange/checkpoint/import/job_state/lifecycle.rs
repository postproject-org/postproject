use postproject_core::{JobId, JobState, ObjectRef, RevisionEventType, RevisionId};
use postproject_protocol::{JobOperation, JobTransition};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    ExchangeResult,
    exchange::{job_capture, records::validate_job_completion},
    sqlite_error,
};

use super::{expected, invalid, observed_claim, require, store, with_state};

pub(in crate::exchange::checkpoint::import) fn changed(
    connection: &Connection,
    change: &JobTransition,
    revision: RevisionId,
    sequence: u64,
    floor: u64,
) -> ExchangeResult<()> {
    let job = change.job_id();
    require(connection, ObjectRef::Job(job), floor)?;
    if sequence <= floor
        || change
            .input_boundary()
            .is_some_and(|boundary| boundary > sequence)
    {
        return Err(invalid().into());
    }
    if let Some(prior) = expected(connection, job)? {
        if prior.state() != change.previous() {
            return Err(invalid().into());
        }
    } else {
        baseline(connection, job, change.previous(), floor)?;
    }
    let current = job_capture::header(connection, job)?;
    let prior_claim = claim(connection, job, floor)?;
    if let JobState::Succeeded(completion) = change.state() {
        if prior_claim.is_none_or(|claim| {
            change
                .input_boundary()
                .is_none_or(|boundary| boundary > claim)
        }) {
            return Err(invalid().into());
        }
        super::super::media_state::require(
            connection,
            ObjectRef::Representation(completion.representation_id()),
            floor,
        )?;
        super::super::activity_state::require(
            connection,
            ObjectRef::Activity(completion.activity_id()),
            floor,
        )?;
        validate_job_completion(connection, revision, &current, completion)?;
    }
    let claim = match change.operation() {
        JobOperation::Claim => Some(sequence),
        JobOperation::Renew => prior_claim,
        _ => None,
    };
    store(
        connection,
        &with_state(&current, change.state().clone())?,
        false,
        claim,
    )
}

fn claim(connection: &Connection, job: JobId, floor: u64) -> ExchangeResult<Option<u64>> {
    let expected: Option<Option<i64>> = connection
        .query_row(
            "SELECT claim_sequence FROM checkpoint_job_state WHERE job = ?1",
            [job.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error("read authored current claim boundary"))?;
    match expected {
        Some(claim) => claim
            .map(|sequence| {
                crate::stored_u64(sequence, "authored claim boundary").map_err(Into::into)
            })
            .transpose(),
        None => observed_claim(connection, job, floor),
    }
}

fn baseline(
    connection: &Connection,
    job: JobId,
    previous: &JobState,
    floor: u64,
) -> ExchangeResult<()> {
    if floor == 0 {
        return Err(invalid().into());
    }
    let kinds = [
        RevisionEventType::JobRequested,
        RevisionEventType::JobClaimed,
        RevisionEventType::JobClaimRenewed,
        RevisionEventType::JobClaimReleased,
        RevisionEventType::JobSucceeded,
        RevisionEventType::JobFailed,
        RevisionEventType::JobCancelled,
    ]
    .map(crate::stored_revision_event_kind)
    .into_iter()
    .collect::<postproject_core::Result<Vec<_>>>()?;
    let latest: Option<i64> = connection.query_row("SELECT e.kind FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.primary_id = ?1 AND r.sequence <= ?2 AND e.kind IN (?3, ?4, ?5, ?6, ?7, ?8, ?9) ORDER BY r.sequence DESC, e.position DESC LIMIT 1", params![job.as_bytes().as_slice(), i64::try_from(floor).map_err(|_| invalid())?, kinds[0], kinds[1], kinds[2], kinds[3], kinds[4], kinds[5], kinds[6]], |row| row.get(0))
        .optional().map_err(sqlite_error("read baseline job lifecycle observation"))?;
    let compatible = match latest {
        None => true, // An unobserved migration baseline is not reconstructed.
        Some(kind) if kind == kinds[0] || kind == kinds[3] => {
            matches!(previous, JobState::Requested)
        }
        Some(kind) if kind == kinds[1] || kind == kinds[2] => {
            matches!(previous, JobState::Claimed(_))
        }
        Some(kind) if kind == kinds[4] => matches!(previous, JobState::Succeeded(_)),
        Some(kind) if kind == kinds[5] => matches!(previous, JobState::Failed(_)),
        Some(kind) if kind == kinds[6] => matches!(previous, JobState::Cancelled),
        _ => false,
    };
    if !compatible {
        return Err(invalid().into());
    }
    Ok(())
}
