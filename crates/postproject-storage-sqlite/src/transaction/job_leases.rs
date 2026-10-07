//! Authority-checked lease operations inside a domain edit.

use std::{sync::Arc, time::Duration};

use postproject_core::{JobLeaseState, validate_job_lease_duration};

use super::{
    Activity, AgentIdentity, Error, ErrorKind, JobClaimId, JobFailure, JobId, OptionalExtension,
    RepresentationImport, Result, SqliteTransaction, Timestamp, ToolIdentity, TransactionState,
    params, sqlite_error,
};
use crate::{SqliteJobLease, job_clock, job_lease::LeaseUpdate};

mod input_guards;

#[cfg(test)]
mod tests;

impl SqliteTransaction<'_> {
    /// Claims work for a checked duration; ownership activates only on commit.
    ///
    /// # Errors
    /// Rejects invalid durations, unavailable jobs, clock rollback and storage errors.
    pub fn claim_job_lease(
        &mut self,
        job: JobId,
        tool: &ToolIdentity,
        agent: Option<&AgentIdentity>,
        duration: Duration,
    ) -> Result<SqliteJobLease> {
        let micros = validate_job_lease_duration(duration)?;
        let now = self.lease_now()?;
        let expiry = lease_expiry(now, micros)?;
        self.guard_claim_inputs(job)?;
        let secret = JobClaimId::new();
        self.claim_job_with_id(job, secret, tool, agent, now, expiry)?;
        let lease = SqliteJobLease::pending(self.production.id(), job, secret, self.id());
        self.lease_guards.push(expiry.as_unix_micros());
        self.lease_updates.push(LeaseUpdate {
            state: Arc::clone(&lease.state),
            after: JobLeaseState::Active { expires_at: expiry },
            newly_claimed: true,
        });
        Ok(lease)
    }

    /// Renews ownership for a duration measured from current authority time.
    ///
    /// # Errors
    /// Rejects closed/wrong-production, expired/superseded claims, non-extending
    /// or invalid durations, clock rollback and storage errors.
    pub fn renew_job_lease(&mut self, lease: &SqliteJobLease, duration: Duration) -> Result<()> {
        let micros = validate_job_lease_duration(duration)?;
        let (now, previous) = self.checked_lease(lease)?;
        let expiry = lease_expiry(now, micros)?;
        self.renew_job_claim(lease.job, lease.secret, now, expiry)?;
        self.update_lease(
            lease,
            previous,
            JobLeaseState::Active { expires_at: expiry },
        );
        Ok(())
    }

    /// Releases a current unexpired claim; local ownership closes on commit.
    ///
    /// # Errors
    /// Rejects invalid ownership, expired/superseded claims, clock or storage errors.
    pub fn release_job_lease(&mut self, lease: &SqliteJobLease) -> Result<()> {
        let (_, expiry) = self.checked_lease(lease)?;
        self.release_job_claim(lease.job, lease.secret)?;
        self.update_lease(lease, expiry, JobLeaseState::Closed);
        Ok(())
    }

    /// Fails work through current unexpired ownership; no output is published.
    ///
    /// # Errors
    /// Rejects invalid ownership, expired/superseded claims, clock or storage errors.
    pub fn fail_job_lease(&mut self, lease: &SqliteJobLease, failure: &JobFailure) -> Result<()> {
        let (now, expiry) = self.checked_lease(lease)?;
        self.fail_job(lease.job, lease.secret, now, failure)?;
        self.update_lease(lease, expiry, JobLeaseState::Closed);
        Ok(())
    }

    /// Publishes the requested output and producing activity atomically.
    ///
    /// Earlier metadata in this edit participates in the same final commit.
    ///
    /// # Errors
    /// Rejects invalid ownership/output/provenance, expired/superseded claims,
    /// clock rollback and storage errors. Completion-specific facts roll back
    /// on staging failure; commit failure rolls back the entire edit.
    pub fn complete_job_lease(
        &mut self,
        lease: &SqliteJobLease,
        output: &RepresentationImport,
        activity: &Activity,
    ) -> Result<()> {
        let (now, expiry) = self.checked_lease(lease)?;
        self.guard_completion_inputs(lease.job)?;
        self.complete_job(lease.job, lease.secret, now, output, activity)?;
        self.update_lease(lease, expiry, JobLeaseState::Closed);
        Ok(())
    }

    pub(crate) fn import_lease(
        &mut self,
        job: JobId,
        secret: JobClaimId,
    ) -> Result<SqliteJobLease> {
        let now = self.lease_now()?;
        let expiry = self.current_lease_expiry(job, secret, now)?;
        self.lease_guards.push(expiry.as_unix_micros());
        Ok(SqliteJobLease::imported(
            self.production.id(),
            job,
            secret,
            expiry,
        ))
    }

    fn checked_lease(&mut self, lease: &SqliteJobLease) -> Result<(Timestamp, Timestamp)> {
        self.lifecycle.ensure_open()?;
        if lease.production != self.production.id() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "job lease belongs to another production",
            ));
        }
        match lease.state()? {
            JobLeaseState::Active { .. } => {}
            JobLeaseState::Pending if lease.owner == Some(self.id()) => {}
            _ => {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "job lease ownership is not active in this edit",
                ));
            }
        }
        let now = self.lease_now()?;
        let expiry = self.current_lease_expiry(lease.job, lease.secret, now)?;
        Ok((now, expiry))
    }

    fn current_lease_expiry(
        &mut self,
        job: JobId,
        secret: JobClaimId,
        now: Timestamp,
    ) -> Result<Timestamp> {
        let expiry: Option<i64> = self
            .open_transaction()?
            .query_row(
                "SELECT claim_expires_at_micros FROM jobs
             WHERE id = ?1 AND state = 2 AND claim_id = ?2
               AND claim_expires_at_micros > ?3",
                params![
                    job.as_bytes().as_slice(),
                    secret.as_bytes().as_slice(),
                    now.as_unix_micros()
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error("validate job lease"))?;
        expiry.map(Timestamp::from_unix_micros).ok_or_else(|| {
            Error::new(
                ErrorKind::Conflict,
                "job lease is expired, superseded or no longer claimed",
            )
        })
    }

    fn update_lease(
        &mut self,
        lease: &SqliteJobLease,
        guarded_expiry: Timestamp,
        after: JobLeaseState,
    ) {
        self.lease_guards.push(guarded_expiry.as_unix_micros());
        self.lease_updates.push(LeaseUpdate {
            state: Arc::clone(&lease.state),
            after,
            newly_claimed: false,
        });
    }

    fn lease_now(&mut self) -> Result<Timestamp> {
        self.lifecycle.ensure_open()?;
        let now = self.lease_authority.clock.now()?;
        self.lease_high_water = Some(
            self.lease_high_water
                .map_or(now.as_unix_micros(), |previous| {
                    previous.max(now.as_unix_micros())
                }),
        );
        job_clock::observe(self.open_transaction()?, now)?;
        Ok(now)
    }

    pub(super) fn check_lease_commit(&mut self) -> Result<()> {
        if self.lease_high_water.is_some() {
            let now = self.lease_now()?.as_unix_micros();
            if self.lease_guards.iter().any(|expiry| *expiry <= now) {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "job lease expired before commit",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn finish_leases(&mut self, committed: bool) {
        for update in self.lease_updates.drain(..) {
            update.finish(committed);
        }
        self.lease_guards.clear();
    }

    pub(super) fn persist_failed_lease_time(&mut self) -> Result<()> {
        if let Some(high_water) = self.lease_high_water.take() {
            let connection = crate::open_connection(&self.lease_authority.path)?;
            job_clock::persist(&connection, high_water)?;
        }
        Ok(())
    }
}

impl Drop for SqliteTransaction<'_> {
    fn drop(&mut self) {
        self.transaction.take();
        self.finish_leases(false);
        // Explicit commit/rollback reports bookkeeping failures. A destructor
        // cannot report them and must never panic or commit domain facts.
        if self.lifecycle.state() != TransactionState::Committed {
            let _ = self.persist_failed_lease_time();
        }
    }
}

fn lease_expiry(now: Timestamp, micros: u64) -> Result<Timestamp> {
    let micros = i64::try_from(micros)
        .map_err(|_| Error::new(ErrorKind::InvalidArgument, "job lease duration overflow"))?;
    now.as_unix_micros()
        .checked_add(micros)
        .map(Timestamp::from_unix_micros)
        .ok_or_else(|| Error::new(ErrorKind::InvalidArgument, "job lease expiry overflow"))
}
