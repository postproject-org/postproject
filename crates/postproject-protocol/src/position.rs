//! Portable replay positions and persistent genesis/migration anchors.

use postproject_core::{DecisionBase, RevisionId};
use serde_json::json;

use crate::{
    Digest, DigestDomain, Document, ProtocolBase, Result, Scope,
    fields::{exact, malformed, nullable, object},
};

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
        if sequence > i64::MAX as u64 {
            return Err(malformed());
        }
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
        if base.decision().sequence() > i64::MAX as u64 {
            return Err(malformed());
        }
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

    /// Encodes the exact continuation, including its source history and digest.
    #[must_use]
    pub fn document(self) -> Document {
        Document {
            value: json!({
                "production":self.scope().production().to_string(),
                "history":self.scope().history().to_string(),
                "revision":self.revision().map(|id| id.to_string()),
                "sequence":self.sequence().to_string(),"digest":self.digest().to_string()
            }),
        }
    }

    /// Decodes a continuation without claiming that its revision exists.
    ///
    /// # Errors
    /// Rejects unknown fields, noncanonical identities/digests and contradictory
    /// or out-of-range revision boundaries.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &["production", "history", "revision", "sequence", "digest"],
        )?;
        Self::new(
            Scope::new(exact(&fields["production"])?, exact(&fields["history"])?),
            nullable(&fields["revision"], exact)?,
            exact(&fields["sequence"])?,
            exact(&fields["digest"])?,
        )
    }
}
