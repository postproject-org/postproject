//! Capture mutable observations inside the operation's rollback boundary.

use postproject_core::{
    Activity, AgentIdentity, JobClaimId, JobFailure, JobId, RepresentationImport, Result,
    Timestamp, ToolIdentity,
};
use postproject_protocol::{JobOperation, JobTransition};

use super::{SqliteTransaction, next_revision_sequence};
use crate::{
    exchange::{CapturedEffect, job_capture},
    sqlite_error, stored_u64,
};

impl SqliteTransaction<'_> {
    fn stage_job_change(
        &mut self,
        job: JobId,
        operation: JobOperation,
        now: Option<Timestamp>,
        mutate: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        self.stage_media_atomically(|this| {
            let before = job_capture::header(this.open_transaction()?, job)?;
            let boundary = match operation {
                JobOperation::Claim => Some(this.base_revision.map_or(
                    stored_u64(
                        next_revision_sequence(this.open_transaction()?)?,
                        "claim input boundary",
                    )?,
                    |(_, sequence)| sequence,
                )),
                JobOperation::Complete => Some(this.completion_input_boundary(job)?),
                _ => None,
            };
            mutate(this)?;
            let after = job_capture::header(this.open_transaction()?, job)?;
            let change = JobTransition::new(
                job,
                operation,
                before.state().clone(),
                after.state().clone(),
                now,
                boundary,
            )
            .map_err(|_| {
                postproject_core::Error::new(
                    postproject_core::ErrorKind::Internal,
                    "cannot capture job transition",
                )
            })?;
            this.pending_effects
                .push(CapturedEffect::JobChanged(Box::new(change)));
            Ok(())
        })
    }

    fn completion_input_boundary(&mut self, job: JobId) -> Result<u64> {
        let sequence = if self.pending_events.iter().any(|event| {
            matches!(event,
            postproject_core::RevisionEventKind::JobClaimed { job_id } if *job_id == job)
        }) {
            next_revision_sequence(self.open_transaction()?)?
        } else {
            self.open_transaction()?
                .query_row(
                    "SELECT r.sequence FROM revisions r
                JOIN revision_events e ON e.revision_id = r.id
                WHERE e.kind = 21 AND e.primary_id = ?1 ORDER BY r.sequence DESC LIMIT 1",
                    [job.as_bytes().as_slice()],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(sqlite_error("capture completion input boundary"))?
        };
        let sequence = stored_u64(sequence, "completion input boundary")?;
        Ok(self
            .base_revision
            .map_or(sequence, |(_, base)| base.min(sequence)))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "retain private authority-selected claim facts"
    )]
    pub(super) fn claim_job_with_id(
        &mut self,
        job: JobId,
        secret: JobClaimId,
        tool: &ToolIdentity,
        agent: Option<&AgentIdentity>,
        now: Timestamp,
        expiry: Timestamp,
    ) -> Result<()> {
        self.stage_job_change(job, JobOperation::Claim, Some(now), |this| {
            this.claim_job_with_id_inner(job, secret, tool, agent, now, expiry)
        })
    }

    pub(super) fn renew_job_claim(
        &mut self,
        job: JobId,
        secret: JobClaimId,
        now: Timestamp,
        expiry: Timestamp,
    ) -> Result<()> {
        self.stage_job_change(job, JobOperation::Renew, Some(now), |this| {
            this.renew_job_claim_inner(job, secret, now, expiry)
        })
    }

    pub(super) fn release_job_claim(
        &mut self,
        job: JobId,
        secret: JobClaimId,
        now: Timestamp,
    ) -> Result<()> {
        self.stage_job_change(job, JobOperation::Release, Some(now), |this| {
            this.release_job_claim_inner(job, secret)
        })
    }

    pub(super) fn fail_job(
        &mut self,
        job: JobId,
        secret: JobClaimId,
        now: Timestamp,
        failure: &JobFailure,
    ) -> Result<()> {
        self.stage_job_change(job, JobOperation::Fail, Some(now), |this| {
            this.fail_job_inner(job, secret, now, failure)
        })
    }

    pub(super) fn complete_job(
        &mut self,
        job: JobId,
        secret: JobClaimId,
        now: Timestamp,
        output: &RepresentationImport,
        activity: &Activity,
    ) -> Result<()> {
        self.stage_job_change(job, JobOperation::Complete, Some(now), |this| {
            this.complete_job_staged(job, secret, now, output, activity)
        })
    }

    /// Cancels requested or claimed work, capturing its original observed state.
    ///
    /// # Errors
    /// Rejects missing/terminal jobs and storage or transaction failures.
    pub fn cancel_job(&mut self, job: JobId) -> Result<()> {
        self.stage_job_change(job, JobOperation::Cancel, None, |this| {
            this.cancel_job_inner(job)
        })
    }
}
