//! Portable replay positions and persistent genesis/migration anchors.

use postproject_core::{DecisionBase, RevisionId};
use serde_json::json;

use crate::{Digest, DigestDomain, Document, ProtocolBase, Result, Scope};

/// A complete applied revision boundary in one source history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    base: ProtocolBase,
    digest: Digest,
}

impl Position {
    /// Constructs a complete scoped boundary without authorizing its existence.
    ///
    /// # Errors
    /// Rejects contradictory genesis/revision fields or a wrong production base.
    pub fn new(
        scope: Scope,
        revision: Option<RevisionId>,
        sequence: u64,
        digest: Digest,
    ) -> Result<Self> {
        let decision =
            crate::fields::checked(DecisionBase::new(scope.production(), revision, sequence))?;
        Ok(Self {
            base: ProtocolBase::new(scope, decision)?,
            digest,
        })
    }

    /// Creates the deterministic continuation anchor for one retained floor.
    ///
    /// Storage persists this floor once; later checkpoint IDs do not affect it.
    ///
    /// # Errors
    /// Rejects invalid scope or an internally unencodable anchor.
    pub fn anchor(base: ProtocolBase) -> Result<Self> {
        let document = Document {
            value: json!({
                "kind":"anchor","version":"1","production":base.scope().production().to_string(),
                "history":base.scope().history().to_string(),
                "revision":base.decision().revision_id().map(|id| id.to_string()),
                "sequence":base.decision().sequence().to_string()
            }),
        };
        Ok(Self {
            base,
            digest: document.digest(DigestDomain::Anchor)?,
        })
    }

    /// Returns source production/history identity.
    #[must_use]
    pub const fn scope(self) -> Scope {
        self.base.scope()
    }
    /// Returns the source revision identity, absent only at genesis.
    #[must_use]
    pub const fn revision(self) -> Option<RevisionId> {
        self.base.decision().revision_id()
    }
    /// Returns the source revision sequence.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.base.decision().sequence()
    }
    /// Returns integrity relative to the source chain anchor.
    #[must_use]
    pub const fn digest(self) -> Digest {
        self.digest
    }
    /// Returns a mutation base while retaining its original source scope.
    #[must_use]
    pub const fn decision_base(self) -> ProtocolBase {
        self.base
    }
}
