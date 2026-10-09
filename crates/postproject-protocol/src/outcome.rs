//! Stable public results for identified decoded proposals.

mod job;
mod wire;
pub use job::{JobResult, JobResultState};

use postproject_core::CommitReceipt;

use crate::{
    ClientId, Digest, Document, Extensions, Proposal, Rejection, RequestId, Result, Scope,
    fields::malformed,
};

/// A terminal result retained independently of current domain state.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OutcomeStatus {
    /// Successful changing or no-change commit with its own receipt.
    Accepted(CommitReceipt),
    /// The whole decoded request was rejected without domain changes.
    Rejected(Rejection),
}

/// A public result bound to complete normalized intent and scoped identity.
///
/// This value never contains or recreates an owning worker capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outcome {
    scope: Scope,
    client: ClientId,
    request: RequestId,
    request_digest: Digest,
    extensions: Extensions,
    status: OutcomeStatus,
}

impl Outcome {
    /// Builds an accepted result from the transaction's actual receipt.
    ///
    /// # Errors
    /// Rejects a receipt from another production or unsupported request intent.
    pub fn accepted(proposal: &Proposal, receipt: CommitReceipt) -> Result<Self> {
        Self::new(proposal, OutcomeStatus::Accepted(receipt))
    }

    /// Builds a terminal result preserving the request's identity/extensions.
    ///
    /// # Errors
    /// Rejects unsupported future request intent.
    pub fn rejected(proposal: &Proposal, rejection: Rejection) -> Result<Self> {
        Self::new(proposal, OutcomeStatus::Rejected(rejection))
    }

    fn new(proposal: &Proposal, status: OutcomeStatus) -> Result<Self> {
        Self::from_parts(
            proposal.scope(),
            proposal.client(),
            proposal.request(),
            proposal.digest()?,
            proposal.extensions().clone(),
            status,
        )
    }

    pub(super) fn from_parts(
        scope: Scope,
        client: ClientId,
        request: RequestId,
        request_digest: Digest,
        extensions: Extensions,
        status: OutcomeStatus,
    ) -> Result<Self> {
        if matches!(&status, OutcomeStatus::Accepted(receipt) if receipt.production_id() != scope.production())
        {
            return Err(malformed());
        }
        Ok(Self {
            scope,
            client,
            request,
            request_digest,
            extensions,
            status,
        })
    }

    /// Encodes the complete public result with an explicit terminal alternative.
    ///
    /// # Errors
    /// Rejects invalid receipt boundaries or future unsupported alternatives.
    pub fn document(&self) -> Result<Document> {
        wire::encode(self)
    }

    /// Decodes public results without importing capabilities or applying effects.
    ///
    /// # Errors
    /// Rejects unknown critical fields/features or invalid scoped receipt data.
    pub fn from_document(document: &Document) -> Result<Self> {
        wire::decode(document)
    }

    /// Returns production/history scope.
    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }
    /// Returns client identity.
    #[must_use]
    pub const fn client(&self) -> ClientId {
        self.client
    }
    /// Returns the retained request identity.
    #[must_use]
    pub const fn request(&self) -> RequestId {
        self.request
    }
    /// Returns the original complete normalized public request digest.
    #[must_use]
    pub const fn request_digest(&self) -> Digest {
        self.request_digest
    }
    /// Borrows exact preserved extension facts.
    #[must_use]
    pub const fn extensions(&self) -> &Extensions {
        &self.extensions
    }
    /// Borrows the terminal result; accepted no-op has no revision.
    #[must_use]
    pub const fn status(&self) -> &OutcomeStatus {
        &self.status
    }
}
