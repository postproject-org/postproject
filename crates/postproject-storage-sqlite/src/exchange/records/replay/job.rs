//! Faithful passive work observations, never worker authorization.
mod request;

use postproject_core::{JobState, RepresentationId, RevisionEventKind};
use postproject_protocol::{JobHeader, JobInput, JobOperation, JobTransition, RecordManifest};
use rusqlite::{Transaction, params};

use super::{effects::invalid, facts};
use crate::{
    ExchangeResult, exchange::job_capture, sqlite_error, transaction::encode_representation_kind,
};

pub(in crate::exchange) struct JobApply {
    header: JobHeader,
    inputs: u64,
    previous: Option<RepresentationId>,
}

impl JobApply {
    pub(super) fn begin(
        transaction: &Transaction<'_>,
        manifest: &RecordManifest,
        header: JobHeader,
        events: &mut u64,
    ) -> ExchangeResult<Option<Self>> {
        if !matches!(header.state(), JobState::Requested) {
            return Err(invalid().into());
        }
        request::insert(transaction, &header, true)?;
        facts::observation(
            transaction,
            manifest,
            *events,
            &RevisionEventKind::JobRequested {
                job_id: header.id(),
            },
        )?;
        *events += 1;
        Ok((header.input_count() > 0).then_some(Self {
            header,
            inputs: 0,
            previous: None,
        }))
    }

    pub(in crate::exchange) fn input(
        &mut self,
        transaction: &Transaction<'_>,
        input: JobInput,
    ) -> ExchangeResult<bool> {
        let representation = input.representation_id();
        if input.job_id() != self.header.id()
            || input.position() != self.inputs
            || self.inputs >= self.header.input_count()
            || self.previous.is_some_and(|old| old >= representation)
            || !crate::transaction::representation_exists(transaction, representation)?
        {
            return Err(invalid().into());
        }
        transaction
            .execute(
                "INSERT INTO job_inputs (job_id, position, representation_id) VALUES (?1, ?2, ?3)",
                params![
                    input.job_id().as_bytes().as_slice(),
                    i64::try_from(input.position()).map_err(|_| invalid())?,
                    representation.as_bytes().as_slice()
                ],
            )
            .map_err(sqlite_error("stage replayed job input"))?;
        self.previous = Some(representation);
        self.inputs += 1;
        Ok(self.inputs == self.header.input_count())
    }

    pub(in crate::exchange) fn begin_checkpoint(
        transaction: &Transaction<'_>,
        header: JobHeader,
    ) -> ExchangeResult<Option<Self>> {
        request::insert(transaction, &header, false)?;
        persist_state(transaction, header.id(), header.state())?;
        Ok((header.input_count() > 0).then_some(Self {
            header,
            inputs: 0,
            previous: None,
        }))
    }
}

pub(super) fn transition(
    transaction: &Transaction<'_>,
    manifest: &RecordManifest,
    change: &JobTransition,
) -> ExchangeResult<()> {
    let header = facts::structural(job_capture::header(transaction, change.job_id()))?;
    if header.state() != change.previous()
        || change
            .input_boundary()
            .is_some_and(|sequence| sequence > manifest.revision().sequence())
    {
        return Err(invalid().into());
    }
    if let JobState::Succeeded(completion) = change.state() {
        completion_valid(transaction, manifest, &header, completion)?;
    }
    if change.operation() == JobOperation::Complete {
        let latest: Option<i64> = transaction
            .query_row(
                "SELECT MAX(r.sequence) FROM revisions r
            JOIN revision_events e ON e.revision_id = r.id WHERE e.kind = 21 AND e.primary_id = ?1",
                [change.job_id().as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(sqlite_error("validate original claim boundary"))?;
        if latest.is_none_or(|sequence| {
            change
                .input_boundary()
                .is_none_or(|boundary| boundary > u64::try_from(sequence).unwrap_or(0))
        }) {
            return Err(invalid().into());
        }
    }
    persist_state(transaction, change.job_id(), change.state())
}

fn persist_state(
    transaction: &Transaction<'_>,
    job: postproject_core::JobId,
    state: &JobState,
) -> ExchangeResult<()> {
    let (kind, claim, completion, failure) = match state {
        JobState::Requested => (1, None, None, None),
        JobState::Claimed(claim) => (2, Some(claim), None, None),
        JobState::Succeeded(completion) => (3, None, Some(completion), None),
        JobState::Failed(failure) => (4, None, None, Some(failure)),
        JobState::Cancelled => (5, None, None, None),
        _ => return Err(invalid().into()),
    };
    let tool = claim.map(postproject_core::JobClaim::tool);
    let agent = claim.and_then(postproject_core::JobClaim::agent);
    let identifier = agent.and_then(postproject_core::AgentIdentity::identifier);
    let changed = transaction.execute("UPDATE jobs SET state = ?1, claim_inert = ?2, claim_id = NULL,
        claim_tool_name = ?3, claim_tool_version = ?4, claim_tool_uri = ?5,
        claim_agent_name = ?6, claim_agent_scheme = ?7, claim_agent_value = ?8, claim_agent_qualifier = ?9,
        claim_expires_at_micros = ?10, completion_activity_id = ?11, completion_representation_id = ?12,
        failure_diagnostic = ?13 WHERE id = ?14", params![kind, i64::from(claim.is_some()),
            tool.map(postproject_core::ToolIdentity::name), tool.and_then(postproject_core::ToolIdentity::version), tool.and_then(postproject_core::ToolIdentity::uri),
            agent.and_then(postproject_core::AgentIdentity::name), identifier.map(|id| id.scheme().as_str()),
            identifier.map(postproject_core::ExternalIdentifier::value), identifier.and_then(postproject_core::ExternalIdentifier::qualifier),
            claim.map(|claim|claim.expires_at().as_unix_micros()), completion.map(|item|item.activity_id().into_bytes().to_vec()),
            completion.map(|item|item.representation_id().into_bytes().to_vec()), failure.map(postproject_core::JobFailure::diagnostic), job.as_bytes().as_slice()])
        .map_err(sqlite_error("stage inert job state"))?;
    if changed != 1 {
        return Err(invalid().into());
    }
    Ok(())
}

fn completion_valid(
    transaction: &Transaction<'_>,
    manifest: &RecordManifest,
    header: &JobHeader,
    completion: &postproject_core::JobCompletion,
) -> ExchangeResult<()> {
    let output = header.requested_output();
    let valid: bool = transaction.query_row("SELECT
        EXISTS(SELECT 1 FROM representations WHERE id = ?1 AND asset_id = ?2 AND kind = ?3)
        AND EXISTS(SELECT 1 FROM revision_events WHERE revision_id = ?4 AND kind = 2 AND primary_id = ?1)
        AND EXISTS(SELECT 1 FROM revision_events WHERE revision_id = ?4 AND kind = 11 AND primary_id = ?5)
        AND (SELECT count(*) FROM activity_outputs WHERE activity_id = ?5) = 1
        AND EXISTS(SELECT 1 FROM activity_outputs WHERE activity_id = ?5 AND representation_id = ?1)
        AND (SELECT count(*) FROM activity_inputs WHERE activity_id = ?5) = (SELECT count(*) FROM job_inputs WHERE job_id = ?6)
        AND NOT EXISTS(SELECT 1 FROM job_inputs j WHERE j.job_id = ?6 AND
            (SELECT count(*) FROM activity_inputs a WHERE a.activity_id = ?5 AND a.representation_id = j.representation_id) != 1)",
        params![completion.representation_id().as_bytes().as_slice(), output.asset_id().as_bytes().as_slice(),
            encode_representation_kind(output.representation_kind())?, manifest.revision().id().as_bytes().as_slice(),
            completion.activity_id().as_bytes().as_slice(), header.id().as_bytes().as_slice()], |row| row.get(0))
        .map_err(sqlite_error("validate replayed job publication"))?;
    if !valid {
        return Err(invalid().into());
    }
    Ok(())
}
