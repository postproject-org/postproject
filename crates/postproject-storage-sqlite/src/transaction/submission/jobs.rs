//! Native worker operations keep their authority clock, input and claim guards.

use std::collections::BTreeMap;

use postproject_core::{Error, ErrorKind, JobId, JobLeaseState, Result};
use postproject_protocol::{Command, Outcome};

use super::{
    SqliteTransaction,
    capabilities::{Capability, CapabilityInput},
};
use crate::{LocalSubmissionResult, SqliteJobLease};

pub(super) struct StagedCapabilities<'a, 'b> {
    input: &'a CapabilityInput<'b>,
    imported: BTreeMap<JobId, SqliteJobLease>,
    claimed: Vec<SqliteJobLease>,
}

impl<'a, 'b> StagedCapabilities<'a, 'b> {
    pub fn new(input: &'a CapabilityInput<'b>) -> Self {
        Self {
            input,
            imported: BTreeMap::new(),
            claimed: vec![],
        }
    }

    fn lease(&mut self, edit: &mut SqliteTransaction<'_>, job: JobId) -> Result<&SqliteJobLease> {
        if let Some(lease) = self
            .claimed
            .iter()
            .rev()
            .find(|lease| lease.job_id() == job)
        {
            return Ok(lease);
        }
        match self.input.get(job)? {
            Capability::Borrowed(lease) => Ok(lease),
            Capability::Token { job, secret, .. } => {
                if !self.imported.contains_key(job) {
                    self.imported
                        .insert(*job, edit.import_lease(*job, *secret)?);
                }
                self.imported.get(job).ok_or_else(|| {
                    Error::new(ErrorKind::Internal, "imported job ownership disappeared")
                })
            }
        }
    }

    pub fn finish(self, outcome: Outcome) -> Result<LocalSubmissionResult> {
        let mut delivered = Vec::new();
        for lease in self.claimed {
            if matches!(lease.state()?, JobLeaseState::Active { .. }) {
                delivered.push(lease);
            }
        }
        Ok(LocalSubmissionResult::new(outcome, delivered))
    }
}

pub(super) fn stage(
    edit: &mut SqliteTransaction<'_>,
    command: &Command,
    capabilities: &mut StagedCapabilities<'_, '_>,
) -> Result<bool> {
    match command {
        Command::RequestJob(job) => edit.request_job(job)?,
        Command::ClaimJob {
            job_id,
            tool,
            agent,
            duration,
        } => {
            capabilities.claimed.push(edit.claim_job_lease(
                *job_id,
                tool,
                agent.as_ref(),
                *duration,
            )?);
        }
        Command::RenewJob { job_id, duration } => {
            let lease = capabilities.lease(edit, *job_id)?;
            edit.renew_job_lease(lease, *duration)?;
        }
        Command::ReleaseJob(job_id) => {
            let lease = capabilities.lease(edit, *job_id)?;
            edit.release_job_lease(lease)?;
        }
        Command::FailJob { job_id, failure } => {
            let lease = capabilities.lease(edit, *job_id)?;
            edit.fail_job_lease(lease, failure)?;
        }
        Command::CompleteJob {
            job_id,
            output,
            activity,
        } => {
            let lease = capabilities.lease(edit, *job_id)?;
            edit.complete_job_lease(lease, output, activity)?;
        }
        Command::CancelJob(job_id) => edit.cancel_job(*job_id)?,
        _ => return Ok(false),
    }
    Ok(true)
}
