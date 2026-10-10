//! Owned public recovery values, never owning worker capability.

use postproject_protocol::{
    ClientId, HistoryId, Outcome, OutcomeStatus, RejectionKind, RequestId, Scope,
};
use postproject_storage_sqlite::ExchangeResult;

use super::{PpRequestIdentity, error};
use crate::{
    PpCommitReceipt, PpError, PpProduction, initialize_output, initialize_value, invalid_argument,
    lock_production, require_output,
};

/// Immutable copied public result; safe to retain independently of its store.
pub struct PpOutcome {
    pub(super) outcome: Outcome,
    bytes: Vec<u8>,
}

impl PpOutcome {
    pub(super) fn new(outcome: Outcome) -> ExchangeResult<Self> {
        let bytes = outcome.document()?.canonical_bytes()?;
        Ok(Self { outcome, bytes })
    }
}

/// Looks up the original terminal public result after an uncertain response.
///
/// Absence returns `NotFound`. Lookup neither checks nor returns worker ownership;
/// retrying a private proposal still requires its original private binding.
///
/// # Safety
/// Production live/serialized, output writable, error null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_submission_outcome(
    production: *const PpProduction,
    identity: *const PpRequestIdentity,
    out_outcome: *mut *mut PpOutcome,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output initialized and nullable input checked before borrowing.
    unsafe {
        initialize_output(out_outcome);
        error::call(out_error, || {
            require_output(out_outcome, "out_outcome")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let identity = identity
                .as_ref()
                .ok_or_else(|| invalid_argument("request identity must not be null"))?;
            let scope = Scope::new(
                crate::ProductionId::from_bytes(identity.production_id.bytes),
                HistoryId::from_bytes(identity.history_id.bytes),
            );
            let outcome = lock_production(&production.state)
                .submission_outcome(
                    scope,
                    ClientId::from_bytes(identity.client_id.bytes),
                    RequestId::from_bytes(identity.request_id.bytes),
                )?
                .ok_or_else(|| {
                    crate::Error::new(
                        crate::ErrorKind::NotFound,
                        "request has no retained outcome",
                    )
                })?;
            out_outcome.write(Box::into_raw(Box::new(PpOutcome::new(outcome)?)));
            Ok(())
        })
    }
}

/// Reads the terminal alternative: one accepted, two rejected.
///
/// # Safety
/// Outcome live, output writable, error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_status(
    outcome: *const PpOutcome,
    out_status: *mut u32,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear output before checking nullable handle.
    unsafe {
        initialize_value(out_status, 0);
        crate::ffi_call(out_error, || {
            require_output(out_status, "out_status")?;
            let outcome = outcome
                .as_ref()
                .ok_or_else(|| invalid_argument("outcome must not be null"))?;
            out_status.write(match outcome.outcome.status() {
                OutcomeStatus::Accepted(_) => 1,
                OutcomeStatus::Rejected(_) => 2,
                _ => {
                    return Err(crate::Error::new(
                        crate::ErrorKind::Unsupported,
                        "unknown outcome alternative",
                    ));
                }
            });
            Ok(())
        })
    }
}

/// Copies this request's accepted receipt; rejection clears output and fails.
///
/// # Safety
/// Outcome live, receipt output writable, error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_receipt(
    outcome: *const PpOutcome,
    out_receipt: *mut PpCommitReceipt,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output initialized before selecting the checked alternative.
    unsafe {
        initialize_value(
            out_receipt,
            PpCommitReceipt::from(&crate::CommitReceipt::new(
                crate::ProductionId::from_bytes([0; 16]),
                None,
            )),
        );
        crate::ffi_call(out_error, || {
            require_output(out_receipt, "out_receipt")?;
            let outcome = outcome
                .as_ref()
                .ok_or_else(|| invalid_argument("outcome must not be null"))?;
            let OutcomeStatus::Accepted(receipt) = outcome.outcome.status() else {
                return Err(invalid_argument("outcome is not accepted"));
            };
            out_receipt.write(receipt.into());
            Ok(())
        })
    }
}

/// Copies a terminal rejection as an owned error, or null for accepted intent.
///
/// Successful inspection returns OK even when the owned rejection has a failure
/// code. Its conflict detail uses the existing error accessors and lifetime.
///
/// # Safety
/// Outcome live, rejection output writable, error optional. Release returned
/// rejection exactly once with `pp_error_release`.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_rejection(
    outcome: *const PpOutcome,
    out_rejection: *mut *mut PpError,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear owning output before reading immutable terminal facts.
    unsafe {
        initialize_output(out_rejection);
        crate::ffi_call(out_error, || {
            require_output(out_rejection, "out_rejection")?;
            let outcome = outcome
                .as_ref()
                .ok_or_else(|| invalid_argument("outcome must not be null"))?;
            if let OutcomeStatus::Rejected(rejection) = outcome.outcome.status() {
                let code = match rejection.kind() {
                    RejectionKind::InvalidBase => 15,
                    RejectionKind::Domain(kind) => crate::error_code(kind),
                    _ => crate::PP_ERROR_UNSUPPORTED,
                };
                crate::write_error(
                    out_rejection,
                    code,
                    "retained proposal rejection",
                    rejection
                        .conflict()
                        .and_then(crate::AbiTransactionConflict::from_domain),
                );
            }
            Ok(())
        })
    }
}

/// Borrows canonical public bytes until outcome release, without any credential.
///
/// # Safety
/// Outcome live, both outputs writable, error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_document(
    outcome: *const PpOutcome,
    out_bytes: *mut *const u8,
    out_length: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs cleared before handle validation; bytes remain owned.
    unsafe {
        crate::initialize_const_output(out_bytes);
        initialize_value(out_length, 0);
        crate::ffi_call(out_error, || {
            require_output(out_bytes, "out_bytes")?;
            require_output(out_length, "out_length")?;
            let outcome = outcome
                .as_ref()
                .ok_or_else(|| invalid_argument("outcome must not be null"))?;
            out_bytes.write(outcome.bytes.as_ptr());
            out_length.write(crate::length_as_u64(outcome.bytes.len())?);
            Ok(())
        })
    }
}

/// Releases a public copy without changing durable submission or worker state.
///
/// # Safety
/// Pointer null or uniquely owned, unreleased, not concurrently used.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_release(outcome: *mut PpOutcome) {
    if !outcome.is_null() {
        let _ = crate::catch_unwind(crate::AssertUnwindSafe(|| {
            // SAFETY: Caller transfers unique owning allocation once.
            drop(unsafe { Box::from_raw(outcome) });
        }));
    }
}
