//! Owned coherent read contexts and production-bound edit construction.

use crate::{
    Arc, DecisionBase, Error, PpError, PpProduction, PpProductionId, PpTransaction, PpUuid,
    ProductionId, ProductionState, RevisionId, begin_transaction_handle, ffi_call,
    initialize_output, initialize_value, invalid_argument, lock_production, production_handle,
    require_output,
};

/// Opaque caller-serialized coherent view retaining its production for edits.
pub struct PpReadSession {
    pub(crate) reader: PpProduction,
    base: DecisionBase,
    state: Arc<ProductionState>,
}

/// Copyable stack representation of a detached production decision base.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpDecisionBase {
    /// Production whose view was read.
    pub production_id: PpProductionId,
    /// One for a revision, zero for the initial empty journal.
    pub has_revision: u8,
    /// Revision identity when present.
    pub revision_id: PpUuid,
    /// Revision sequence, zero only for an empty journal.
    pub revision_sequence: u64,
}

impl From<DecisionBase> for PpDecisionBase {
    fn from(base: DecisionBase) -> Self {
        Self {
            production_id: PpProductionId {
                bytes: base.production_id().into_bytes(),
            },
            has_revision: u8::from(base.revision_id().is_some()),
            revision_id: PpUuid {
                bytes: base.revision_id().map_or([0; 16], RevisionId::into_bytes),
            },
            revision_sequence: base.sequence(),
        }
    }
}

impl TryFrom<PpDecisionBase> for DecisionBase {
    type Error = Error;
    fn try_from(base: PpDecisionBase) -> Result<Self, Self::Error> {
        if base.has_revision > 1 || (base.has_revision == 0 && base.revision_id.bytes != [0; 16]) {
            return Err(invalid_argument(
                "invalid decision base revision tag or payload",
            ));
        }
        Self::new(
            ProductionId::from_bytes(base.production_id.bytes),
            (base.has_revision == 1).then(|| RevisionId::from_bytes(base.revision_id.bytes)),
            base.revision_sequence,
        )
    }
}

/// Opens and pins a read view before returning it to the caller.
///
/// # Safety
///
/// Production must be live, `out_session` writable, and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_read_session(
    production: *const PpProduction,
    out_session: *mut *mut PpReadSession,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller guarantees pointer validity; nullable inputs are checked.
    unsafe {
        initialize_output(out_session);
        ffi_call(out_error, || {
            require_output(out_session, "out_session")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let view = lock_production(&production.state).read_session()?;
            let base = view.decision_base();
            out_session.write(Box::into_raw(Box::new(PpReadSession {
                reader: production_handle(view.into_read_only()),
                base,
                state: Arc::clone(&production.state),
            })));
            Ok(())
        })
    }
}

/// Copies a detached base which remains valid after session release.
///
/// # Safety
///
/// Session must be live, `out_base` writable, and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_decision_base(
    session: *const PpReadSession,
    out_base: *mut PpDecisionBase,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output validity is a caller requirement; initialize before validation.
    unsafe {
        initialize_value(
            out_base,
            PpDecisionBase {
                production_id: PpProductionId { bytes: [0; 16] },
                has_revision: 0,
                revision_id: PpUuid { bytes: [0; 16] },
                revision_sequence: 0,
            },
        );
        ffi_call(out_error, || {
            require_output(out_base, "out_base")?;
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            out_base.write(session.base.into());
            Ok(())
        })
    }
}

/// Begins an edit carrying the session's base without upgrading its read view.
///
/// # Safety
///
/// Session must be live, `out_transaction` writable, and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_begin_edit(
    session: *const PpReadSession,
    out_transaction: *mut *mut PpTransaction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Initialize outputs before checking caller-provided nullable handles.
    unsafe {
        initialize_output(out_transaction);
        ffi_call(out_error, || {
            require_output(out_transaction, "out_transaction")?;
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            out_transaction.write(begin_transaction_handle(
                &session.state,
                None,
                Some(session.base),
            )?);
            Ok(())
        })
    }
}

/// Begins an edit from detached, untrusted scoped context.
///
/// # Safety
///
/// Production/base must be live/readable, outputs writable or `out_error` null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_begin_edit(
    production: *const PpProduction,
    base: *const PpDecisionBase,
    out_transaction: *mut *mut PpTransaction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointer validity is required; values are copied and checked.
    unsafe {
        initialize_output(out_transaction);
        ffi_call(out_error, || {
            require_output(out_transaction, "out_transaction")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let base = base
                .as_ref()
                .ok_or_else(|| invalid_argument("base must not be null"))?;
            out_transaction.write(begin_transaction_handle(
                &production.state,
                None,
                Some((*base).try_into()?),
            )?);
            Ok(())
        })
    }
}

/// Releases a pinned view; null is a no-op and no write is performed.
///
/// # Safety
///
/// A non-null session must be live, exclusively accessed and released once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_release(session: *mut PpReadSession) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !session.is_null() {
            // SAFETY: Ownership is transferred back exactly once by contract.
            drop(unsafe { Box::from_raw(session) });
        }
    }));
}
