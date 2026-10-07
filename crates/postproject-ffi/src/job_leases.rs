//! Shared native lease ownership across lazy transaction staging.

use std::{sync::{Arc, Mutex}, time::Duration};

use postproject_core::{JobLeaseState, TransactionId};
use postproject_storage_sqlite::{SqliteJobLease, SqliteTransaction};

use crate::{Activity, AgentIdentity, Error, JobFailure, JobId, PpTransaction,
    ProductionId, RepresentationImport, StagedMutation, ToolIdentity, invalid_argument, lock_production};

/// Opaque owning job lease. Freeing it never changes durable job state.
pub struct PpJobLease {
    pub(crate) shared: Arc<SharedLease>,
}

pub(crate) struct SharedLease {
    pub production: ProductionId,
    pub job: JobId,
    inner: Mutex<LeaseOwner>,
}

enum LeaseOwner {
    Pending(TransactionId),
    Ready(SqliteJobLease),
    Closed,
}

pub(crate) enum LeaseMutation {
    Claim { lease: Arc<SharedLease>, tool: ToolIdentity, agent: Option<AgentIdentity>, duration: Duration },
    Renew(Arc<SharedLease>, Duration),
    Release(Arc<SharedLease>),
    Fail(Arc<SharedLease>, JobFailure),
    Complete { lease: Arc<SharedLease>, output: RepresentationImport, activity: Box<Activity> },
}

impl SharedLease {
    pub fn pending(transaction: &PpTransaction, job: JobId) -> Arc<Self> {
        Arc::new(Self {
            production: lock_production(&transaction.state).production().id(),
            job,
            inner: Mutex::new(LeaseOwner::Pending(transaction.lifecycle.id())),
        })
    }

    pub fn imported(lease: SqliteJobLease) -> Arc<Self> {
        Arc::new(Self { production: lease.production_id(), job: lease.job_id(), inner: Mutex::new(LeaseOwner::Ready(lease)) })
    }

    pub fn state(&self) -> Result<JobLeaseState, Error> {
        let owner = self.inner.lock().map_err(|_| ownership_error())?;
        match &*owner {
            LeaseOwner::Pending(_) => Ok(JobLeaseState::Pending),
            LeaseOwner::Ready(lease) => lease.state(),
            LeaseOwner::Closed => Ok(JobLeaseState::Closed),
        }
    }

    pub fn token(&self) -> Result<String, Error> {
        let owner = self.inner.lock().map_err(|_| ownership_error())?;
        match &*owner {
            LeaseOwner::Ready(lease) => lease.export_token(),
            _ => Err(invalid_argument("only a committed active lease can be exported")),
        }
    }

    pub fn validate_edit(&self, transaction: &PpTransaction) -> Result<(), Error> {
        transaction.lifecycle.ensure_open()?;
        if self.production != lock_production(&transaction.state).production().id() {
            return Err(invalid_argument("job lease belongs to another production"));
        }
        let owner = self.inner.lock().map_err(|_| ownership_error())?;
        match &*owner {
            LeaseOwner::Pending(id) if *id == transaction.lifecycle.id() => Ok(()),
            LeaseOwner::Ready(lease) if matches!(lease.state()?, JobLeaseState::Active { .. }) => Ok(()),
            _ => Err(Error::new(postproject_core::ErrorKind::Conflict, "job lease ownership is not active in this edit")),
        }
    }
}

impl LeaseMutation {
    pub fn apply(&self, transaction: &mut SqliteTransaction<'_>) -> Result<(), Error> {
        if let Self::Claim { lease, tool, agent, duration } = self {
            let mut owner = lease.inner.lock().map_err(|_| ownership_error())?;
            if !matches!(&*owner, LeaseOwner::Pending(_)) {
                return Err(ownership_error());
            }
            *owner = LeaseOwner::Ready(transaction.claim_job_lease(lease.job, tool, agent.as_ref(), *duration)?);
            return Ok(());
        }
        let shared = match self {
            Self::Renew(lease, _) | Self::Release(lease) | Self::Fail(lease, _) | Self::Complete { lease, .. } => lease,
            Self::Claim { .. } => unreachable!("claim handled above"),
        };
        let owner = shared.inner.lock().map_err(|_| ownership_error())?;
        let LeaseOwner::Ready(lease) = &*owner else { return Err(ownership_error()); };
        match self {
            Self::Renew(_, duration) => transaction.renew_job_lease(lease, *duration),
            Self::Release(_) => transaction.release_job_lease(lease),
            Self::Fail(_, failure) => transaction.fail_job_lease(lease, failure),
            Self::Complete { output, activity, .. } => transaction.complete_job_lease(lease, output, activity),
            Self::Claim { .. } => unreachable!("claim handled above"),
        }
    }
}

pub(crate) fn close_pending(mutations: &[StagedMutation]) {
    for mutation in mutations {
        if let StagedMutation::Lease(LeaseMutation::Claim { lease, .. }) = mutation {
            let mut owner = lease.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if matches!(&*owner, LeaseOwner::Pending(_)) { *owner = LeaseOwner::Closed; }
            // A started SQLite edit owns pending activation/rollback cleanup.
        }
    }
}

fn ownership_error() -> Error {
    Error::new(postproject_core::ErrorKind::Internal, "job lease ownership state is unavailable")
}
