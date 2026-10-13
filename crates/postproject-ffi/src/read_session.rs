//! Owned coherent read contexts and production-bound edit construction.

use crate::{
    Arc, DecisionBase, Error, PpError, PpProduction, PpProductionId, PpRevisionId, PpTransaction,
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
    pub revision_id: PpRevisionId,
    /// Revision sequence, zero only for an empty journal.
    pub revision_sequence: u64,
}

/// Optional borrowed attribution for edit creation.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpEditContext {
    /// Origin name, or null for absent attribution.
    pub origin_name: *const std::ffi::c_char,
    /// Optional origin version; requires a name.
    pub origin_version: *const std::ffi::c_char,
    /// Optional origin URI; requires a name.
    pub origin_uri: *const std::ffi::c_char,
    /// Optional revision message.
    pub message: *const std::ffi::c_char,
}

unsafe fn edit_context(context: *const PpEditContext) -> Result<crate::RevisionContext, Error> {
    // SAFETY: The caller supplies a readable context with borrowed UTF-8 strings.
    unsafe {
        let Some(context) = context.as_ref() else {
            return Ok(crate::RevisionContext::default());
        };
        let name = crate::optional_utf8(context.origin_name, "origin_name")?;
        let version = crate::optional_utf8(context.origin_version, "origin_version")?;
        let uri = crate::optional_utf8(context.origin_uri, "origin_uri")?;
        let origin = match name {
            Some(name) => Some(crate::OriginIdentity::new(
                name,
                version.map(str::to_owned),
                uri.map(str::to_owned),
            )?),
            None if version.is_none() && uri.is_none() => None,
            None => {
                return Err(invalid_argument(
                    "origin_version and origin_uri require origin_name",
                ));
            }
        };
        crate::RevisionContext::new(
            origin,
            crate::optional_utf8(context.message, "message")?.map(str::to_owned),
        )
    }
}

impl From<DecisionBase> for PpDecisionBase {
    fn from(base: DecisionBase) -> Self {
        Self {
            production_id: PpProductionId {
                bytes: base.production_id().into_bytes(),
            },
            has_revision: u8::from(base.revision_id().is_some()),
            revision_id: PpRevisionId {
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

/// Parses a bounded canonical token; store membership is checked when editing.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_decision_base_parse(
    text: *const std::ffi::c_char,
    out_base: *mut PpDecisionBase,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized before validating caller inputs.
    unsafe {
        initialize_value(
            out_base,
            PpDecisionBase {
                production_id: PpProductionId { bytes: [0; 16] },
                has_revision: 0,
                revision_id: PpRevisionId { bytes: [0; 16] },
                revision_sequence: 0,
            },
        );
        ffi_call(out_error, || {
            require_output(out_base, "out_base")?;
            let text = crate::required_utf8(text, "decision-base token")?;
            let base: DecisionBase = text.parse()?;
            out_base.write(base.into());
            Ok(())
        })
    }
}

/// Formats a validated base as an owned canonical token, released with string release.
///
/// # Safety
/// Base must be readable, output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_decision_base_format(
    base: *const PpDecisionBase,
    out_text: *mut *mut std::ffi::c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output initialized and pointer checked before reading.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let base = base
                .as_ref()
                .ok_or_else(|| invalid_argument("base must not be null"))?;
            let base = DecisionBase::try_from(*base)?;
            out_text
                .write(crate::exact_cstring(&base.to_string(), "decision-base token")?.into_raw());
            Ok(())
        })
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
                revision_id: PpRevisionId { bytes: [0; 16] },
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
/// Session must be live, context null or readable with borrowed UTF-8 strings,
/// `out_transaction` writable, and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_begin_edit(
    session: *const PpReadSession,
    context: *const PpEditContext,
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
            let context = edit_context(context)?;
            let transaction = begin_transaction_handle(&session.state, None, Some(session.base))?;
            (*transaction).revision_context = context;
            // Retain the exact knowledge used by automatic content observations.
            (*transaction).decision_reader = Some(Arc::clone(&session.reader.state));
            out_transaction.write(transaction);
            Ok(())
        })
    }
}

/// Begins an edit from detached, untrusted scoped context.
///
/// # Safety
///
/// Production must be live; base and context null or readable (strings UTF-8).
/// Outputs must be writable or `out_error` null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_begin_edit(
    production: *const PpProduction,
    base: *const PpDecisionBase,
    context: *const PpEditContext,
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
            let base = base.as_ref().map(|base| (*base).try_into()).transpose()?;
            let context = edit_context(context)?;
            let transaction = begin_transaction_handle(&production.state, None, base)?;
            (*transaction).revision_context = context;
            out_transaction.write(transaction);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::CString, ptr};

    #[test]
    fn invalid_context_does_not_reserve_an_edit_and_valid_context_is_copied() {
        let directory = tempfile::tempdir().unwrap();
        let store =
            crate::SqliteProduction::create(directory.path().join("context.pproj"), None).unwrap();
        let production = production_handle(store);
        let name = CString::new("test host").unwrap();
        let version = CString::new("1.2").unwrap();
        let message = CString::new("Import clip").unwrap();
        let mut context = PpEditContext {
            origin_name: ptr::null(),
            origin_version: version.as_ptr(),
            origin_uri: ptr::null(),
            message: message.as_ptr(),
        };
        let mut transaction = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All handles and borrowed strings are live; outputs are local.
        unsafe {
            assert_eq!(
                pp_production_begin_edit(
                    &production,
                    ptr::null(),
                    &context,
                    &raw mut transaction,
                    &raw mut error
                ),
                crate::PP_ERROR_INVALID_ARGUMENT
            );
            assert!(transaction.is_null());
            crate::pp_error_release(error);
            context.origin_name = name.as_ptr();
            assert_eq!(
                pp_production_begin_edit(
                    &production,
                    ptr::null(),
                    &context,
                    &raw mut transaction,
                    &raw mut error
                ),
                crate::PP_OK
            );
            assert!(!transaction.is_null());
            assert_eq!(
                (*transaction).revision_context.origin().unwrap().name(),
                "test host"
            );
            assert_eq!(
                (*transaction).revision_context.message(),
                Some("Import clip")
            );
            drop(name);
            drop(version);
            drop(message);
            assert_eq!(
                (*transaction).revision_context.origin().unwrap().version(),
                Some("1.2")
            );
            crate::pp_transaction_release(transaction);
            assert_eq!(
                pp_production_begin_edit(
                    &production,
                    ptr::null(),
                    ptr::null(),
                    &raw mut transaction,
                    &raw mut error
                ),
                crate::PP_OK
            );
            crate::pp_transaction_release(transaction);
        }
    }
}
