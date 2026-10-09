//! Public recovery and privileged local delivery have distinct lifetimes.

use postproject_protocol::Outcome;

use crate::SqliteJobLease;

/// A retained public outcome and any newly committed local worker ownership.
///
/// Duplicate recovery contains no leases. Dropping this result never releases,
/// renews or reissues ownership; lost delivery needs expiry or cancellation.
#[derive(Debug)]
pub struct LocalSubmissionResult {
    outcome: Outcome,
    leases: Vec<SqliteJobLease>,
}

impl LocalSubmissionResult {
    pub(crate) const fn new(outcome: Outcome, leases: Vec<SqliteJobLease>) -> Self {
        Self { outcome, leases }
    }

    /// Borrows the public terminal result, without a worker credential.
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// Borrows newly committed ownership delivered by this fresh submission.
    #[must_use]
    pub fn leases(&self) -> &[SqliteJobLease] {
        &self.leases
    }

    /// Separates public recovery from newly delivered owning lease handles.
    #[must_use]
    pub fn into_parts(self) -> (Outcome, Vec<SqliteJobLease>) {
        (self.outcome, self.leases)
    }
}
