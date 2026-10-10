//! Native projection of checked public exchange values and ownership.

mod error;
mod outcome;
mod prepare;
mod state;
mod submission;
#[cfg(test)]
mod submission_tests;
#[cfg(test)]
mod tests;

use postproject_protocol::{
    ClientId, Extensions, FailureKind, Proposal, ProtocolBase, ProtocolError, RequestId, Scope,
};
use postproject_storage_sqlite::ExchangeResult;

pub use outcome::PpOutcome;
pub use state::{PpExchangeLimits, PpExchangePosition, PpExchangeState};
pub use submission::{PpPrivateLeaseToken, PpSubmission};

use crate::{
    PpError, PpTransaction, initialize_output, initialize_value, invalid_argument, lock_production,
    require_output,
};

/// Persistent source-history identity, distinct from a production ID.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpHistoryId {
    /// Stable UUID bytes; existence is checked by exchange operations.
    pub bytes: [u8; 16],
}

/// Stable submitting client identity; create once and retain across retries.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpClientId {
    /// Stable UUID bytes, without authentication semantics.
    pub bytes: [u8; 16],
}

/// Stable identity of one complete intent; reuse after an uncertain response.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpRequestId {
    /// Stable UUID bytes; a retained identity must keep its original intent.
    pub bytes: [u8; 16],
}

/// Detached domain decision retaining its original source history.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpProtocolBase {
    /// Production and checked revision/sequence of the observed decision.
    pub decision: crate::PpDecisionBase,
    /// Source history; never replace this with the destination's history.
    pub history_id: PpHistoryId,
}

/// Complete public request identity for durable outcome recovery.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpRequestIdentity {
    /// Original source production, not a destination file identity.
    pub production_id: crate::PpProductionId,
    /// Original source-history generation.
    pub history_id: PpHistoryId,
    /// Stable submitting client.
    pub client_id: PpClientId,
    /// Stable complete-intent request.
    pub request_id: PpRequestId,
}

impl PpRequestIdentity {
    fn empty() -> Self {
        Self {
            production_id: crate::PpProductionId { bytes: [0; 16] },
            history_id: PpHistoryId { bytes: [0; 16] },
            client_id: PpClientId { bytes: [0; 16] },
            request_id: PpRequestId { bytes: [0; 16] },
        }
    }
}

/// Immutable checked public intent, owning its exact transport bytes.
pub struct PpProposal {
    pub(crate) proposal: Proposal,
    bytes: Vec<u8>,
}

impl PpProposal {
    fn new(proposal: Proposal) -> ExchangeResult<Self> {
        let mut result = Self {
            proposal,
            bytes: Vec::new(),
        };
        result.bytes = result.proposal.document()?.canonical_bytes()?;
        Ok(result)
    }
}

/// Parses bounded portable intent; ordinary native commands use typed staging.
///
/// # Safety
/// Bytes readable for the accepted length, output writable, error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_proposal_parse(
    bytes: *const u8,
    length: u64,
    out_proposal: *mut *mut PpProposal,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Bound length before constructing a borrowed caller slice.
    unsafe {
        initialize_output(out_proposal);
        error::call(out_error, || {
            require_output(out_proposal, "out_proposal")?;
            let limits = postproject_protocol::Limits::default();
            let length = usize::try_from(length)
                .ok()
                .filter(|length| *length <= limits.max_bytes())
                .ok_or_else(|| {
                    ProtocolError::new(
                        FailureKind::LimitExceeded,
                        "proposal input exceeds document limit",
                    )
                })?;
            if bytes.is_null() {
                return Err(invalid_argument("proposal bytes must not be null").into());
            }
            let document = postproject_protocol::Document::parse(
                std::slice::from_raw_parts(bytes, length),
                limits,
            )?;
            let proposal = PpProposal::new(Proposal::from_document(&document)?)?;
            out_proposal.write(Box::into_raw(Box::new(proposal)));
            Ok(())
        })
    }
}

/// Copies the complete public recovery identity; no credential is returned.
///
/// # Safety
/// Proposal live, output writable, error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_proposal_identity(
    proposal: *const PpProposal,
    out_identity: *mut PpRequestIdentity,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear copied output before borrowing the nullable handle.
    unsafe {
        initialize_value(out_identity, PpRequestIdentity::empty());
        crate::ffi_call(out_error, || {
            require_output(out_identity, "out_identity")?;
            let proposal = proposal
                .as_ref()
                .ok_or_else(|| invalid_argument("proposal must not be null"))?;
            out_identity.write(PpRequestIdentity {
                production_id: crate::PpProductionId {
                    bytes: proposal.proposal.scope().production().into_bytes(),
                },
                history_id: PpHistoryId {
                    bytes: *proposal.proposal.scope().history().as_bytes(),
                },
                client_id: PpClientId {
                    bytes: *proposal.proposal.client().as_bytes(),
                },
                request_id: PpRequestId {
                    bytes: *proposal.proposal.request().as_bytes(),
                },
            });
            Ok(())
        })
    }
}

impl TryFrom<PpProtocolBase> for ProtocolBase {
    type Error = postproject_storage_sqlite::ExchangeError;

    fn try_from(base: PpProtocolBase) -> Result<Self, Self::Error> {
        let decision = base.decision.try_into()?;
        Ok(Self::new(
            Scope::new(
                crate::ProductionId::from_bytes(base.decision.production_id.bytes),
                postproject_protocol::HistoryId::from_bytes(base.history_id.bytes),
            ),
            decision,
        )?)
    }
}

/// Converts checked staged intent without writing a domain revision.
///
/// Success closes the transaction and pending native claims. Failure leaves it
/// open. A supplied source base must match the edit; a null base binds the edit's
/// local decision to its checked opened history. Legacy revision-only edits must
/// be recreated with a decision base before conversion.
///
/// # Safety
/// Transaction must be live/serialized, optional base readable, output writable,
/// and error null or writable. The returned proposal is released once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_prepare_proposal(
    transaction: *mut PpTransaction,
    client_id: PpClientId,
    request_id: PpRequestId,
    base: *const PpProtocolBase,
    out_proposal: *mut *mut PpProposal,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Initialize outputs and validate nullable pointers before use.
    unsafe {
        initialize_output(out_proposal);
        error::call(out_error, || {
            require_output(out_proposal, "out_proposal")?;
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            if transaction.base_revision.is_some() {
                return Err(invalid_argument(
                    "proposal preparation requires a scoped decision edit",
                )
                .into());
            }
            if transaction.mutations.len() > postproject_protocol::MAX_PROPOSAL_COMMANDS {
                return Err(ProtocolError::new(
                    FailureKind::LimitExceeded,
                    "proposal command limit exceeded",
                )
                .into());
            }
            let mut source = lock_production(&transaction.state);
            let scope = source.exchange_scope()?;
            let decision = transaction.decision_base;
            let base = if let Some(base) = base.as_ref() {
                let base = ProtocolBase::try_from(*base)?;
                if base.scope() != scope {
                    return Err(ProtocolError::new(
                        FailureKind::ScopeMismatch,
                        "proposal decision belongs to another history",
                    )
                    .into());
                }
                if Some(base.decision()) != decision {
                    return Err(ProtocolError::new(
                        FailureKind::InvalidBase,
                        "proposal decision differs from the staging edit",
                    )
                    .into());
                }
                Some(base)
            } else {
                decision
                    .map(|decision| ProtocolBase::new(scope, decision))
                    .transpose()?
            };
            if let Some(decision) = decision {
                source.begin_edit(decision)?;
            }
            let proposal = Proposal::new(
                scope,
                ClientId::from_bytes(client_id.bytes),
                RequestId::from_bytes(request_id.bytes),
                base,
                transaction.revision_context.clone(),
                transaction
                    .mutations
                    .iter()
                    .map(prepare::command)
                    .collect::<postproject_protocol::Result<Vec<_>>>()?,
                Extensions::default(),
            )?;
            let proposal = Box::new(PpProposal::new(proposal)?);
            drop(source);
            transaction.rollback()?;
            out_proposal.write(Box::into_raw(proposal));
            Ok(())
        })
    }
}

/// Borrows canonical public bytes until proposal release; never contains tokens.
///
/// # Safety
/// Proposal must be live, both outputs writable, error null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_proposal_document(
    proposal: *const PpProposal,
    out_bytes: *mut *const u8,
    out_length: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs initialized before validating readable handle.
    unsafe {
        crate::initialize_const_output(out_bytes);
        initialize_value(out_length, 0);
        crate::ffi_call(out_error, || {
            require_output(out_bytes, "out_bytes")?;
            require_output(out_length, "out_length")?;
            let proposal = proposal
                .as_ref()
                .ok_or_else(|| invalid_argument("proposal must not be null"))?;
            out_bytes.write(proposal.bytes.as_ptr());
            out_length.write(crate::length_as_u64(proposal.bytes.len())?);
            Ok(())
        })
    }
}

/// Releases immutable intent without changing any durable request outcome.
///
/// # Safety
/// Proposal must be null or an unreleased owning pointer, not in concurrent use.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_proposal_release(proposal: *mut PpProposal) {
    if !proposal.is_null() {
        let _ = crate::catch_unwind(crate::AssertUnwindSafe(|| {
            // SAFETY: Caller transfers the allocation's unique ownership once.
            drop(unsafe { Box::from_raw(proposal) });
        }));
    }
}
