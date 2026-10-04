//! Production-scoped bases for decisions made from coherent read views.

use crate::{Error, ErrorKind, ProductionId, ProductionRead, ProductionStore, Result, RevisionId};

/// Detached optimistic context; it does not retain a read view or a lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecisionBase {
    production_id: ProductionId,
    revision_id: Option<RevisionId>,
    sequence: u64,
}

impl DecisionBase {
    /// Validates a production-scoped revision observation.
    ///
    /// `None` with sequence zero represents the initial empty journal. The
    /// store validates membership and the revision/sequence pair when editing.
    ///
    /// # Errors
    ///
    /// Returns invalid argument for contradictory empty/revision values.
    pub fn new(
        production_id: ProductionId,
        revision_id: Option<RevisionId>,
        sequence: u64,
    ) -> Result<Self> {
        if revision_id.is_some() != (sequence != 0) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "a decision revision requires a positive sequence; an empty base requires zero",
            ));
        }
        Ok(Self {
            production_id,
            revision_id,
            sequence,
        })
    }

    /// Returns the production whose facts were read.
    #[must_use]
    pub const fn production_id(self) -> ProductionId {
        self.production_id
    }

    /// Returns the read view's revision, absent only for an empty journal.
    #[must_use]
    pub const fn revision_id(self) -> Option<RevisionId> {
        self.revision_id
    }

    /// Returns the read view's revision sequence, zero only for an empty journal.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

/// An owned coherent production view with a base captured by the same read.
pub trait ProductionReadSession {
    /// Returns only domain read operations on the retained view.
    fn read(&self) -> &dyn ProductionRead;

    /// Detaches the view's production-scoped decision context.
    fn decision_base(&self) -> DecisionBase;

    /// Begins an edit carrying this view's decision base automatically.
    ///
    /// The store owns a fresh write transaction, not an upgraded read view.
    ///
    /// # Errors
    ///
    /// Rejects wrong production scope, invalid revisions or storage failures.
    fn edit<'production, Store: ProductionStore>(
        &self,
        store: &'production mut Store,
    ) -> Result<Store::Transaction<'production>> {
        store.begin_edit(self.decision_base())
    }
}
