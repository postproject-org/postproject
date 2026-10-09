//! Complete normalized request intent, independent of authoritative effects.

mod wire;

use std::collections::BTreeSet;

use postproject_core::RevisionContext;

use crate::{
    ClientId, Command, Digest, DigestDomain, Document, Extensions, FailureKind, ProtocolBase,
    ProtocolError, RecordFeature, RequestId, Result, Scope,
};

/// Maximum number of top-level commands in one portable proposal.
pub const MAX_PROPOSAL_COMMANDS: usize = 1000;

/// One ordered semantic request with a stable client/request identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    scope: Scope,
    client: ClientId,
    request: RequestId,
    base: Option<ProtocolBase>,
    context: RevisionContext,
    commands: Vec<Command>,
    extensions: Extensions,
}

impl Proposal {
    /// Builds bounded intent; current-state and required-base checks belong to
    /// authority submission so a terminal domain rejection can be retained.
    ///
    /// # Errors
    /// Rejects mismatched base scope, oversized commands or JSON bytes/nodes/depth.
    pub fn new(
        scope: Scope,
        client: ClientId,
        request: RequestId,
        base: Option<ProtocolBase>,
        context: RevisionContext,
        commands: Vec<Command>,
        extensions: Extensions,
    ) -> Result<Self> {
        if commands.len() > MAX_PROPOSAL_COMMANDS {
            return Err(crate::fields::limit());
        }
        if base.is_some_and(|base| base.scope() != scope) {
            return Err(ProtocolError::new(
                FailureKind::ScopeMismatch,
                "proposal base belongs to another history",
            ));
        }
        let proposal = Self {
            scope,
            client,
            request,
            base,
            context,
            commands,
            extensions,
        };
        proposal
            .document()?
            .bounded_canonical_bytes(crate::Limits::default())?;
        Ok(proposal)
    }

    /// Decodes required features and checked domain intent.
    ///
    /// # Errors
    /// Rejects unsupported versions/features, unknown fields and malformed input.
    pub fn from_document(document: &Document) -> Result<Self> {
        wire::decode(&document.value)
    }

    /// Encodes the normalized complete request, preserving ordered commands.
    ///
    /// # Errors
    /// Rejects any unsupported future command/value kind.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: wire::encode(self)?,
        })
    }

    /// Returns the normalized complete public request digest.
    ///
    /// Private capability binding, when required, is a separate authority fact.
    ///
    /// # Errors
    /// Rejects unsupported future command/value kinds.
    pub fn digest(&self) -> Result<Digest> {
        self.document()?.digest(DigestDomain::Request)
    }

    /// Returns source production/history scope.
    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }
    /// Returns the stable client instance identity.
    #[must_use]
    pub const fn client(&self) -> ClientId {
        self.client
    }
    /// Returns the stable identity reused for outcome recovery.
    #[must_use]
    pub const fn request(&self) -> RequestId {
        self.request
    }
    /// Returns the optional scoped detached decision.
    #[must_use]
    pub const fn base(&self) -> Option<ProtocolBase> {
        self.base
    }
    /// Returns application attribution/message, without authentication semantics.
    #[must_use]
    pub const fn context(&self) -> &RevisionContext {
        &self.context
    }
    /// Borrows commands in their original semantic order.
    #[must_use]
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }
    /// Returns the exact lexical set of codecs used by this proposal.
    ///
    /// Empty proposals retain the original metadata profile for compatibility.
    #[must_use]
    pub fn required_features(&self) -> BTreeSet<RecordFeature> {
        let mut features: BTreeSet<_> = self
            .commands
            .iter()
            .flat_map(|command| {
                let mut codecs = BTreeSet::from([command.required_feature()]);
                if matches!(command, Command::CompleteJob { .. }) {
                    codecs.extend([RecordFeature::Media, RecordFeature::Provenance]);
                }
                codecs
            })
            .collect();
        if features.is_empty() {
            features.insert(RecordFeature::Metadata);
        }
        features
    }
    /// Returns exact preserved extension facts.
    #[must_use]
    pub const fn extensions(&self) -> &Extensions {
        &self.extensions
    }
}
