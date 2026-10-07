//! Shared lease duration and local ownership states.

use std::time::Duration;

use crate::{Error, ErrorKind, JobId, ProductionId, Result, Timestamp};

/// Backend-owned, production-bound worker capability; never a claim-ID pair.
///
/// Implementations keep credentials private and omit them from diagnostics.
/// Dropping ownership never renews, releases or writes a claim.
pub trait JobLease {
    /// Returns the production scope without a credential.
    fn production_id(&self) -> ProductionId;
    /// Returns the claimed job without a credential.
    fn job_id(&self) -> JobId;
    /// Returns cached local ownership; transitions always recheck the store.
    ///
    /// # Errors
    /// Returns an ownership-state error when local state cannot be read.
    fn state(&self) -> Result<JobLeaseState>;
    /// Explicitly exports a bounded versioned credential for file/pipe transport.
    ///
    /// # Errors
    /// Rejects pending or closed ownership. Import revalidates current authority.
    fn export_token(&self) -> Result<String>;
}

/// Longest supported job lease: one day.
pub const MAX_JOB_LEASE_DURATION: Duration = Duration::from_secs(86_400);

/// Converts a positive, bounded lease duration to exact whole microseconds.
///
/// # Errors
///
/// Rejects zero, fractional microseconds and durations longer than one day.
pub fn validate_job_lease_duration(duration: Duration) -> Result<u64> {
    if duration.is_zero()
        || duration > MAX_JOB_LEASE_DURATION
        || duration.subsec_nanos() % 1_000 != 0
    {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "job lease duration must be whole microseconds between 1 us and 24 h",
        ));
    }
    u64::try_from(duration.as_micros())
        .map_err(|_| Error::new(ErrorKind::InvalidArgument, "job lease duration overflow"))
}

/// Local ownership state; an active handle still needs authoritative validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobLeaseState {
    /// The claim belongs to an edit that has not committed yet.
    Pending,
    /// The owning edit committed; the timestamp is the last accepted expiry.
    Active {
        /// Unix microseconds, rather than a process-local monotonic instant.
        expires_at: Timestamp,
    },
    /// Ownership ended through a terminal transition or failed claiming edit.
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_conversion_is_exact_and_bounded() {
        for duration in [Duration::ZERO, Duration::from_nanos(1), Duration::MAX] {
            assert_eq!(
                validate_job_lease_duration(duration).unwrap_err().kind(),
                ErrorKind::InvalidArgument
            );
        }
        assert_eq!(
            validate_job_lease_duration(Duration::from_micros(1)).unwrap(),
            1
        );
        assert_eq!(
            validate_job_lease_duration(MAX_JOB_LEASE_DURATION).unwrap(),
            86_400_000_000
        );
        assert!(
            validate_job_lease_duration(MAX_JOB_LEASE_DURATION + Duration::from_micros(1)).is_err()
        );
    }
}
