//! Owning job capabilities; secrets are deliberately absent from Debug.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use postproject_core::{
    Error, ErrorKind, JobClaimId, JobId, JobLeaseState, ProductionId, Result, Timestamp,
    TransactionId,
};

/// A production-bound worker capability. Dropping it never writes to storage.
pub struct SqliteJobLease {
    pub(crate) production: ProductionId,
    pub(crate) job: JobId,
    pub(crate) secret: JobClaimId,
    pub(crate) owner: Option<TransactionId>,
    pub(crate) state: Arc<Mutex<JobLeaseState>>,
}

impl fmt::Debug for SqliteJobLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteJobLease")
            .field("production", &self.production)
            .field("job", &self.job)
            .finish_non_exhaustive()
    }
}

impl SqliteJobLease {
    pub(crate) fn pending(
        production: ProductionId,
        job: JobId,
        secret: JobClaimId,
        owner: TransactionId,
    ) -> Self {
        Self {
            production,
            job,
            secret,
            owner: Some(owner),
            state: Arc::new(Mutex::new(JobLeaseState::Pending)),
        }
    }

    pub(crate) fn imported(
        production: ProductionId,
        job: JobId,
        secret: JobClaimId,
        expires_at: Timestamp,
    ) -> Self {
        Self {
            production,
            job,
            secret,
            owner: None,
            state: Arc::new(Mutex::new(JobLeaseState::Active { expires_at })),
        }
    }

    /// Returns the production scope, without claim credentials.
    #[must_use]
    pub const fn production_id(&self) -> ProductionId {
        self.production
    }

    /// Returns the claimed job, without claim credentials.
    #[must_use]
    pub const fn job_id(&self) -> JobId {
        self.job
    }

    /// Returns cached local ownership state; every mutation rechecks storage.
    ///
    /// # Errors
    /// Returns an internal error if the local state lock is poisoned.
    pub fn state(&self) -> Result<JobLeaseState> {
        self.state
            .lock()
            .map(|state| *state)
            .map_err(|_| state_error())
    }

    /// Explicitly exports a credential for a worker spanning processes.
    ///
    /// Keep the result in a protected file or pipe, never ordinary logs/arguments.
    /// Export is local; import and transitions validate authoritative expiry.
    ///
    /// # Errors
    /// Rejects pending/closed ownership or an internal state failure.
    pub fn export_token(&self) -> Result<String> {
        if !matches!(self.state()?, JobLeaseState::Active { .. }) {
            return Err(Error::new(
                ErrorKind::Conflict,
                "only a committed active lease can be exported",
            ));
        }
        Ok(format!(
            "ppl1:{}:{}:{}",
            self.production, self.job, self.secret
        ))
    }
}

pub(crate) fn parse_token(token: &str) -> Result<(ProductionId, JobId, JobClaimId)> {
    let invalid = || Error::new(ErrorKind::InvalidArgument, "invalid scoped job lease token");
    // Three canonical hyphenated UUIDs, three separators and a four-byte version.
    if token.len() != 115 || !token.is_ascii() {
        return Err(invalid());
    }
    let mut parts = token.split(':');
    if parts.next() != Some("ppl1") {
        return Err(invalid());
    }
    let production: ProductionId = parts
        .next()
        .ok_or_else(invalid)?
        .parse()
        .map_err(|_| invalid())?;
    let job: JobId = parts
        .next()
        .ok_or_else(invalid)?
        .parse()
        .map_err(|_| invalid())?;
    let secret: JobClaimId = parts
        .next()
        .ok_or_else(invalid)?
        .parse()
        .map_err(|_| invalid())?;
    if parts.next().is_some()
        || production.as_bytes() == &[0; 16]
        || job.as_bytes() == &[0; 16]
        || secret.as_bytes() == &[0; 16]
        || token != format!("ppl1:{production}:{job}:{secret}")
    {
        return Err(invalid());
    }
    Ok((production, job, secret))
}

pub(crate) fn state_error() -> Error {
    Error::new(
        ErrorKind::Internal,
        "job lease ownership state is unavailable",
    )
}

pub(crate) struct LeaseUpdate {
    pub state: Arc<Mutex<JobLeaseState>>,
    pub after: JobLeaseState,
    pub newly_claimed: bool,
}

impl LeaseUpdate {
    pub fn finish(&self, committed: bool) {
        // No user code runs under this lock. Recovering poison preserves terminal
        // cleanup without introducing a panic after a durable commit.
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if committed {
            *state = self.after;
        } else if self.newly_claimed {
            *state = JobLeaseState::Closed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_format_is_bounded_canonical_and_redacted() {
        let lease = SqliteJobLease::imported(
            ProductionId::new(),
            JobId::new(),
            JobClaimId::new(),
            Timestamp::from_unix_micros(10),
        );
        let token = lease.export_token().unwrap();
        assert_eq!(
            parse_token(&token).unwrap(),
            (lease.production, lease.job, lease.secret)
        );
        assert!(!format!("{lease:?}").contains(&lease.secret.to_string()));
        for malformed in [
            String::new(),
            "x".repeat(10_000),
            token.to_uppercase(),
            token.replace("ppl1", "ppl2"),
            format!(
                "ppl1:{}:{}:{}",
                ProductionId::from_bytes([0; 16]),
                lease.job,
                lease.secret
            ),
        ] {
            let error = parse_token(&malformed).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert!(!error.to_string().contains(&lease.secret.to_string()));
        }
        let pending = SqliteJobLease::pending(
            lease.production,
            lease.job,
            lease.secret,
            TransactionId::new(),
        );
        assert!(pending.export_token().is_err());
    }
}
