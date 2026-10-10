//! Checked public job observations at the original submission boundary.

use postproject_protocol::{JobResult, JobResultState};

use super::PpOutcome;
use crate::{PpError, PpJobCompletion, PpJobId, invalid_argument, require_output};

unsafe fn job(outcome: *const PpOutcome, index: u64) -> Result<JobResult, crate::Error> {
    // SAFETY: Caller guarantees a live handle when non-null.
    let outcome =
        unsafe { outcome.as_ref() }.ok_or_else(|| invalid_argument("outcome must not be null"))?;
    usize::try_from(index)
        .ok()
        .and_then(|index| outcome.outcome.jobs().get(index))
        .copied()
        .ok_or_else(|| invalid_argument("outcome job index is out of range"))
}

/// Counts noncredential job observations; null returns zero.
///
/// # Safety
/// Outcome null or live, without concurrent release.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_job_count(outcome: *const PpOutcome) -> u64 {
    crate::catch_unwind(crate::AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { outcome.as_ref() }.map_or(0, |value| {
            u64::try_from(value.outcome.jobs().len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Copies stable identity and observed state using the existing `PP_JOB_*` tags.
///
/// # Safety
/// Outcome live, outputs writable, error null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_job_get(
    outcome: *const PpOutcome,
    index: u64,
    out_job: *mut PpJobId,
    out_state: *mut u32,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs cleared and checked before reading the selected job.
    unsafe {
        crate::initialize_value(out_job, PpJobId { bytes: [0; 16] });
        crate::initialize_value(out_state, 0);
        crate::ffi_call(out_error, || {
            require_output(out_job, "out_job")?;
            require_output(out_state, "out_state")?;
            let value = job(outcome, index)?;
            let state = match value.state() {
                JobResultState::Requested => 1,
                JobResultState::Claimed(_) => 2,
                JobResultState::Succeeded(_) => 3,
                JobResultState::Failed => 4,
                JobResultState::Cancelled => 5,
                _ => {
                    return Err(crate::Error::new(
                        crate::ErrorKind::Unsupported,
                        "unknown job result",
                    ));
                }
            };
            out_job.write(PpJobId {
                bytes: *value.job_id().as_bytes(),
            });
            out_state.write(state);
            Ok(())
        })
    }
}

/// Copies expiry only for a claimed observation, without worker ownership.
///
/// # Safety
/// Outcome live, output writable, error null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_job_expiry(
    outcome: *const PpOutcome,
    index: u64,
    out_expiry: *mut i64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear output before checking the state-specific alternative.
    unsafe {
        crate::initialize_value(out_expiry, 0);
        crate::ffi_call(out_error, || {
            require_output(out_expiry, "out_expiry")?;
            let JobResultState::Claimed(expiry) = job(outcome, index)?.state() else {
                return Err(invalid_argument("outcome job is not claimed"));
            };
            out_expiry.write(expiry.as_unix_micros());
            Ok(())
        })
    }
}

/// Copies publication identities only for a succeeded observation.
///
/// # Safety
/// Outcome live, output writable, error null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_outcome_job_completion(
    outcome: *const PpOutcome,
    index: u64,
    out_completion: *mut PpJobCompletion,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear output before checking the state-specific alternative.
    unsafe {
        crate::initialize_value(out_completion, PpJobCompletion::empty());
        crate::ffi_call(out_error, || {
            require_output(out_completion, "out_completion")?;
            let JobResultState::Succeeded(completion) = job(outcome, index)?.state() else {
                return Err(invalid_argument("outcome job is not succeeded"));
            };
            out_completion.write(PpJobCompletion {
                activity_id: crate::PpActivityId {
                    bytes: *completion.activity_id().as_bytes(),
                },
                representation_id: crate::PpRepresentationId {
                    bytes: *completion.representation_id().as_bytes(),
                },
            });
            Ok(())
        })
    }
}
