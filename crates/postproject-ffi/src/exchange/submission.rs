//! Trusted local private context and fresh owning delivery.

use std::{collections::BTreeSet, sync::atomic::Ordering};

use postproject_protocol::MAX_PROPOSAL_COMMANDS;

use super::{PpProposal, error, outcome::PpOutcome};
use crate::{
    PpError, PpJobLease, PpProduction, initialize_output, initialize_value, invalid_argument,
    lock_production, require_output,
};

/// Borrowed exact scoped private token; never part of public intent or outcomes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpPrivateLeaseToken {
    /// Exactly `length` readable bytes, not NUL-terminated.
    pub bytes: *const u8,
    /// Must be exactly 115 for the current checked scoped token.
    pub length: u64,
}

/// Owning local delivery; freeing it never releases a durable claim.
pub struct PpSubmission {
    outcome: PpOutcome,
    leases: Vec<Option<Box<PpJobLease>>>,
}

/// Submits immutable intent with separately borrowed private context.
///
/// Retained retries validate the original binding before current authorization;
/// they return no fresh leases. A changing submission creates one revision.
/// A retained rejection is a successful operation returning a rejected outcome.
/// Storage failure may be uncertain: recover the original identity after reopen.
///
/// # Safety
/// Handles must be live/serialized. Each array is readable for its count (null
/// only with zero count), each token readable for its accepted length. Outputs
/// writable; error optional. Caller owns the returned submission.
#[postproject_ffi_macros::ffi_export]
#[allow(
    clippy::too_many_arguments,
    reason = "separate public intent and two private input channels"
)]
pub unsafe extern "C" fn pp_production_submit_proposal(
    production: *const PpProduction,
    proposal: *const PpProposal,
    leases: *const *const PpJobLease,
    lease_count: u64,
    tokens: *const PpPrivateLeaseToken,
    token_count: u64,
    out_submission: *mut *mut PpSubmission,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Counts bounded before creating slices, then nullable inputs checked.
    unsafe {
        initialize_output(out_submission);
        error::call(out_error, || {
            require_output(out_submission, "out_submission")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let proposal = proposal
                .as_ref()
                .ok_or_else(|| invalid_argument("proposal must not be null"))?;
            let limit = u64::try_from(MAX_PROPOSAL_COMMANDS).unwrap_or(u64::MAX);
            if lease_count.saturating_add(token_count) > limit {
                return Err(
                    invalid_argument("private submission context exceeds its limit").into(),
                );
            }
            let lease_inputs = inputs(leases, lease_count)?;
            let token_inputs = inputs(tokens, token_count)?;
            let mut jobs = BTreeSet::new();
            let leases = lease_inputs
                .iter()
                .map(|lease| {
                    let lease = lease
                        .as_ref()
                        .ok_or_else(|| invalid_argument("private lease must not be null"))?;
                    if !jobs.insert(lease.shared.job) {
                        return Err(invalid_argument("duplicate private lease"));
                    }
                    Ok(lease)
                })
                .collect::<Result<Vec<_>, crate::Error>>()?;
            let tokens = token_inputs
                .iter()
                .map(|token| {
                    if token.length != 115 || token.bytes.is_null() {
                        return Err(invalid_argument("invalid scoped private token"));
                    }
                    std::str::from_utf8(std::slice::from_raw_parts(token.bytes, 115))
                        .map_err(|_| invalid_argument("invalid scoped private token"))
                })
                .collect::<Result<Vec<_>, crate::Error>>()?;
            let mut store = lock_production(&production.state);
            if production.state.transaction_open.load(Ordering::Acquire) {
                return Err(crate::Error::new(
                    crate::ErrorKind::Conflict,
                    "production already has an open transaction",
                )
                .into());
            }
            // Match ordinary commit's source-then-lease lock order. Duplicate
            // native jobs were rejected before acquiring any owning guard.
            let guards = leases
                .iter()
                .map(|lease| lease.shared.borrow_for_submission())
                .collect::<Result<Vec<_>, _>>()?;
            let borrowed = guards
                .iter()
                .map(crate::job_leases::SubmissionLease::native)
                .collect::<Vec<_>>();
            let (outcome, leases) = store
                .submit_proposal_with_capabilities(&proposal.proposal, &borrowed, &tokens)?
                .into_parts();
            let submission = PpSubmission {
                outcome: PpOutcome::new(outcome)?,
                leases: leases
                    .into_iter()
                    .map(|lease| {
                        Some(Box::new(PpJobLease {
                            shared: crate::job_leases::SharedLease::imported(lease),
                        }))
                    })
                    .collect(),
            };
            out_submission.write(Box::into_raw(Box::new(submission)));
            Ok(())
        })
    }
}

unsafe fn inputs<'a, T>(pointer: *const T, count: u64) -> Result<&'a [T], crate::Error> {
    let count = usize::try_from(count).map_err(|_| invalid_argument("input count overflow"))?;
    if count == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(invalid_argument("nonempty input array must not be null"));
    }
    // SAFETY: Caller guarantees readable arrays; exported caller bounds counts.
    Ok(unsafe { std::slice::from_raw_parts(pointer, count) })
}

/// Copies the public outcome without transferring or duplicating any ownership.
///
/// # Safety
/// Submission live/serialized; output writable; error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_submission_outcome(
    submission: *const PpSubmission,
    out_outcome: *mut *mut PpOutcome,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Nullable input validated and owning output initialized.
    unsafe {
        initialize_output(out_outcome);
        error::call(out_error, || {
            require_output(out_outcome, "out_outcome")?;
            let submission = submission
                .as_ref()
                .ok_or_else(|| invalid_argument("submission must not be null"))?;
            out_outcome.write(Box::into_raw(Box::new(PpOutcome::new(
                submission.outcome.outcome.clone(),
            )?)));
            Ok(())
        })
    }
}

/// Returns stable delivery slots; a taken slot remains counted and cannot repeat.
///
/// # Safety
/// Submission must be null or live/serialized.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_submission_lease_count(submission: *const PpSubmission) -> u64 {
    crate::catch_unwind(crate::AssertUnwindSafe(|| {
        // SAFETY: Caller supplies a live handle or null.
        unsafe { submission.as_ref() }
            .map_or(0, |result| u64::try_from(result.leases.len()).unwrap_or(0))
    }))
    .unwrap_or(0)
}

/// Transfers fresh worker ownership once, released with `pp_job_lease_free`.
///
/// # Safety
/// Submission must be live/exclusive, output writable, error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_submission_take_lease(
    submission: *mut PpSubmission,
    index: u64,
    out_lease: *mut *mut PpJobLease,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs initialized before checked ownership transfer.
    unsafe {
        initialize_value(out_lease, std::ptr::null_mut());
        crate::ffi_call(out_error, || {
            require_output(out_lease, "out_lease")?;
            let submission = submission
                .as_mut()
                .ok_or_else(|| invalid_argument("submission must not be null"))?;
            let index =
                usize::try_from(index).map_err(|_| invalid_argument("lease index overflow"))?;
            let slot = submission
                .leases
                .get_mut(index)
                .ok_or_else(|| invalid_argument("lease index out of bounds"))?;
            let lease = slot.take().ok_or_else(|| {
                crate::Error::new(
                    crate::ErrorKind::NotFound,
                    "private lease was already taken",
                )
            })?;
            out_lease.write(Box::into_raw(lease));
            Ok(())
        })
    }
}

/// Releases local delivery without changing any durable job or public outcome.
///
/// # Safety
/// Pointer null or uniquely owned, unreleased, not concurrently used.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_submission_release(submission: *mut PpSubmission) {
    if !submission.is_null() {
        let _ = crate::catch_unwind(crate::AssertUnwindSafe(|| {
            // SAFETY: Caller transfers the allocation's unique ownership once.
            drop(unsafe { Box::from_raw(submission) });
        }));
    }
}
