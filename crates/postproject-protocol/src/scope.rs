//! Protocol identities remain distinct from production and mutation identities.

use std::{fmt, str::FromStr};

use postproject_core::{DecisionBase, ProductionId};
use uuid::Uuid;

use crate::{Document, FailureKind, ProtocolError, Result};

macro_rules! identity {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Uuid);
        impl $name {
            /// Creates a random identity once, before the first submission.
            #[must_use]
            pub fn new() -> Self { Self(Uuid::new_v4()) }
            /// Creates an identity from its stable bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 16]) -> Self { Self(Uuid::from_bytes(bytes)) }
            /// Returns the stable identity bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 16] { self.0.as_bytes() }
        }
        impl Default for $name { fn default() -> Self { Self::new() } }
        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(formatter) }
        }
        impl FromStr for $name {
            type Err = ProtocolError;
            fn from_str(text: &str) -> Result<Self> {
                let id = Uuid::parse_str(text).map_err(|_| crate::fields::malformed())?;
                if id.to_string() != text { return Err(crate::fields::malformed()); }
                Ok(Self(id))
            }
        }
    }
}

identity!(
    /// Persistent authority-history generation.
    HistoryId
);
identity!(
    /// Stable submitting client instance.
    ClientId
);
identity!(
    /// Stable identity of one complete submission intent.
    RequestId
);
identity!(
    /// Local identity of a passive materialization.
    MirrorInstanceId
);
identity!(
    /// Identity of one complete checkpoint export.
    CheckpointId
);

/// Production and source history that scope every exchange operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scope {
    production: ProductionId,
    history: HistoryId,
}

impl Scope {
    /// Creates an explicit source scope; storage verifies its authority.
    #[must_use]
    pub const fn new(production: ProductionId, history: HistoryId) -> Self {
        Self {
            production,
            history,
        }
    }
    /// Returns the production identity.
    #[must_use]
    pub const fn production(self) -> ProductionId {
        self.production
    }
    /// Returns the persistent source-history identity.
    #[must_use]
    pub const fn history(self) -> HistoryId {
        self.history
    }
}

/// A detached domain decision bound to its source authority history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolBase {
    scope: Scope,
    decision: DecisionBase,
}

impl ProtocolBase {
    /// Encodes a detached decision without inventing a replay digest.
    #[must_use]
    pub fn document(self) -> Document {
        Document {
            value: serde_json::json!({
                "production":self.scope.production().to_string(), "history":self.scope.history().to_string(),
                "revision":self.decision.revision_id().map(|id| id.to_string()), "sequence":self.decision.sequence().to_string()
            }),
        }
    }

    /// Decodes exact source scope and checked revision/sequence fields.
    ///
    /// # Errors
    /// Rejects unknown fields, malformed identities/integers and inconsistent
    /// decisions. Storage still verifies history and revision membership.
    pub fn from_document(document: &Document) -> Result<Self> {
        use crate::fields::{checked, exact, nullable, object};
        let fields = object(
            &document.value,
            &["production", "history", "revision", "sequence"],
        )?;
        let scope = Scope::new(exact(&fields["production"])?, exact(&fields["history"])?);
        let decision = checked(DecisionBase::new(
            scope.production(),
            nullable(&fields["revision"], exact)?,
            exact(&fields["sequence"])?,
        ))?;
        Self::new(scope, decision)
    }

    /// Binds a detached decision without discarding source-history scope.
    ///
    /// # Errors
    /// Rejects a decision from another production. Storage additionally verifies
    /// the history generation, actual revision/sequence and migration floor.
    pub fn new(scope: Scope, decision: DecisionBase) -> Result<Self> {
        if scope.production() != decision.production_id() {
            return Err(ProtocolError::new(
                FailureKind::ScopeMismatch,
                "decision belongs to another production",
            ));
        }
        Ok(Self { scope, decision })
    }
    /// Returns the original source scope.
    #[must_use]
    pub const fn scope(self) -> Scope {
        self.scope
    }
    /// Returns the detached local decision, still accompanied by its scope.
    #[must_use]
    pub const fn decision(self) -> DecisionBase {
        self.decision
    }
}
