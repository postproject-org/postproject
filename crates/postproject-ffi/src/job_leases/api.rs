//! C entry points for owning leases and explicit credential transport.

use std::{ffi::c_char, panic::{AssertUnwindSafe, catch_unwind}, sync::Arc, time::Duration};
use postproject_core::{JobLeaseState, validate_job_lease_duration};

use super::{LeaseMutation, PpJobLease, SharedLease};
use crate::{JobId, PpError, PpJobId, PpProduction, PpProductionId, PpTransaction,
    StagedMutation, exact_cstring, ffi_call, initialize_value, invalid_argument,
    lock_production, require_output, worker_identity_from_abi};

/// Claims work for a positive whole-microsecond duration of at most 24 hours.
///
/// # Safety
/// The transaction must be live and exclusively accessed. Strings must be null
/// or valid terminated UTF-8 (tool name required); outputs must be writable.
#[postproject_ffi_macros::ffi_export]
#[allow(clippy::too_many_arguments, reason = "explicit C worker attribution")]
pub unsafe extern "C" fn pp_transaction_claim_job_lease(
    transaction: *mut PpTransaction, job_id: PpJobId,
    tool_name: *const c_char, tool_version: *const c_char, tool_uri: *const c_char,
    agent_name: *const c_char, agent_identifier_scheme: *const c_char,
    agent_identifier_value: *const c_char, agent_identifier_qualifier: *const c_char,
    duration_micros: u64, out_lease: *mut *mut PpJobLease, out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Caller supplies valid pointers; required inputs are checked.
    unsafe {
        initialize_value(out_lease, std::ptr::null_mut());
        ffi_call(out_error, || {
            require_output(out_lease, "out_lease")?;
            let transaction = transaction.as_mut().ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let duration = Duration::from_micros(duration_micros);
            validate_job_lease_duration(duration)?;
            let (tool, agent) = worker_identity_from_abi(tool_name, tool_version, tool_uri,
                agent_name, agent_identifier_scheme, agent_identifier_value, agent_identifier_qualifier)?;
            let shared = SharedLease::pending(transaction, JobId::from_bytes(job_id.bytes));
            transaction.mutations.push(StagedMutation::Lease(LeaseMutation::Claim {
                lease: Arc::clone(&shared), tool, agent, duration,
            }));
            out_lease.write(Box::into_raw(Box::new(PpJobLease { shared })));
            Ok(())
        })
    }
}

/// Imports an exact 115-byte scoped lease token, checking current authority.
///
/// # Safety
/// Production is live/exclusively accessed; token points to `token_size` readable
/// bytes when its size is accepted. Outputs are null or writable (lease required).
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_import_job_lease(
    production: *mut PpProduction, token: *const u8, token_size: u64,
    out_lease: *mut *mut PpJobLease, out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Accepted input size bounds the borrowed slice; ownership is copied.
    unsafe {
        initialize_value(out_lease, std::ptr::null_mut());
        ffi_call(out_error, || {
            require_output(out_lease, "out_lease")?;
            if token_size != 115 || token.is_null() { return Err(invalid_argument("invalid scoped job lease token")); }
            let token = std::str::from_utf8(std::slice::from_raw_parts(token, 115))
                .map_err(|_| invalid_argument("invalid scoped job lease token"))?;
            let production = production.as_mut().ok_or_else(|| invalid_argument("production must not be null"))?;
            let mut store = lock_production(&production.state);
            let shared = SharedLease::imported(store.import_job_lease(token)?);
            out_lease.write(Box::into_raw(Box::new(PpJobLease { shared })));
            Ok(())
        })
    }
}

/// Explicitly exports a credential; release its string with `pp_string_release`.
///
/// # Safety
/// Lease is live; outputs are null or writable (token required). Treat returned
/// credentials as secrets; ordinary logs and process arguments must omit them.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_lease_export_token(
    lease: *const PpJobLease, out_token: *mut *mut c_char, out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Caller supplies live lease and writable output allocation slots.
    unsafe {
        initialize_value(out_token, std::ptr::null_mut());
        ffi_call(out_error, || {
            require_output(out_token, "out_token")?;
            let lease = lease.as_ref().ok_or_else(|| invalid_argument("lease must not be null"))?;
            out_token.write(exact_cstring(&lease.shared.token()?, "job lease token")?.into_raw());
            Ok(())
        })
    }
}

/// Reads local lease scope/state; active expiry is cached, never validation.
///
/// # Safety
/// Lease is live and all outputs are writable (error may be null). Expiry is
/// zero in pending/closed states. Every mutation rechecks authoritative state.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_lease_get(
    lease: *const PpJobLease, out_production: *mut PpProductionId, out_job: *mut PpJobId,
    out_state: *mut u32, out_expiry_unix_micros: *mut i64, out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized before validating required pointers.
    unsafe {
        initialize_value(out_production, PpProductionId { bytes: [0; 16] });
        initialize_value(out_job, PpJobId { bytes: [0; 16] });
        initialize_value(out_state, 0);
        initialize_value(out_expiry_unix_micros, 0);
        ffi_call(out_error, || {
            require_output(out_production, "out_production")?;
            require_output(out_job, "out_job")?;
            require_output(out_state, "out_state")?;
            require_output(out_expiry_unix_micros, "out_expiry_unix_micros")?;
            let lease = lease.as_ref().ok_or_else(|| invalid_argument("lease must not be null"))?;
            let (state, expiry) = match lease.shared.state()? {
                JobLeaseState::Pending => (1, 0),
                JobLeaseState::Active { expires_at } => (2, expires_at.as_unix_micros()),
                JobLeaseState::Closed => (3, 0),
            };
            out_production.write(PpProductionId { bytes: lease.shared.production.into_bytes() });
            out_job.write(PpJobId { bytes: lease.shared.job.into_bytes() });
            out_state.write(state);
            out_expiry_unix_micros.write(expiry);
            Ok(())
        })
    }
}

/// Frees local ownership without releasing the durable claim. Null is a no-op.
///
/// # Safety
/// Non-null lease is live, uniquely freed, and unused concurrently or afterwards.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_lease_free(lease: *mut PpJobLease) {
    if !lease.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: Caller transfers this allocation exactly once.
            drop(unsafe { Box::from_raw(lease) });
        }));
    }
}
