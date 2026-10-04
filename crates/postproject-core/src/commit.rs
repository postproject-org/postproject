//! Atomic results of production commits.

use crate::{ProductionId, Revision};

/// The production and revision actually produced by a successful commit.
///
/// An absent revision is an explicit no-change outcome. It never identifies
/// another transaction's revision, even when the production is nonempty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitReceipt {
    production_id: ProductionId,
    revision: Option<Revision>,
}

impl CommitReceipt {
    /// Creates a receipt from the atomic persistence result.
    #[must_use]
    pub const fn new(production_id: ProductionId, revision: Option<Revision>) -> Self {
        Self {
            production_id,
            revision,
        }
    }

    /// Returns the production whose commit succeeded.
    #[must_use]
    pub const fn production_id(&self) -> ProductionId {
        self.production_id
    }

    /// Returns this commit's new revision, or `None` when no revision was made.
    #[must_use]
    pub const fn revision(&self) -> Option<&Revision> {
        self.revision.as_ref()
    }
}
