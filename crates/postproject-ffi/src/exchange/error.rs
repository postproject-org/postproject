//! Stable numeric protocol failures through the existing owned error channel.

use postproject_protocol::FailureKind;
use postproject_storage_sqlite::{ExchangeError, ExchangeResult};

use crate::{
    AbiTransactionConflict, AssertUnwindSafe, PP_ERROR_INTERNAL, PP_OK, PpError, catch_unwind,
    error_code, initialize_output, panic_message, write_error,
};

pub(super) unsafe fn call(
    out_error: *mut *mut PpError,
    operation: impl FnOnce() -> ExchangeResult<()>,
) -> u32 {
    // SAFETY: The exported operation provides the optional writable error pointer.
    unsafe { initialize_output(out_error) };
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => PP_OK,
        Ok(Err(error)) => {
            let (code, conflict) = match &error {
                ExchangeError::Protocol(error) => (protocol_code(error.kind()), None),
                ExchangeError::Store(error) => (
                    error_code(error.kind()),
                    error
                        .transaction_conflict_detail()
                        .and_then(AbiTransactionConflict::from_domain),
                ),
                _ => (PP_ERROR_INTERNAL, None),
            };
            // SAFETY: Same output contract; diagnostics never include input bytes.
            unsafe { write_error(out_error, code, &error.to_string(), conflict) };
            code
        }
        Err(payload) => {
            // SAFETY: Same optional writable output contract.
            unsafe {
                write_error(
                    out_error,
                    PP_ERROR_INTERNAL,
                    &panic_message(payload.as_ref()),
                    None,
                );
            };
            PP_ERROR_INTERNAL
        }
    }
}

fn protocol_code(kind: FailureKind) -> u32 {
    match kind {
        FailureKind::Unsupported => crate::PP_ERROR_UNSUPPORTED,
        FailureKind::Malformed => 12,
        FailureKind::LimitExceeded => 13,
        FailureKind::ScopeMismatch => 14,
        FailureKind::InvalidBase => 15,
        FailureKind::RequestIdentityMismatch => 16,
        FailureKind::HistoryGap => 17,
        FailureKind::Integrity => 18,
        FailureKind::Divergence => 19,
        FailureKind::MirrorReadOnly => 20,
        _ => PP_ERROR_INTERNAL,
    }
}
