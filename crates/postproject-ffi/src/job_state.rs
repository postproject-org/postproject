//! Checked state-specific job projections.

use std::ffi::c_char;

use crate::jobs::{PP_JOB_CLAIMED, PP_JOB_FAILED, PP_JOB_SUCCEEDED};
use crate::{
    PpError, PpJob, PpJobClaim, PpJobCompletion, PpJobSet, ffi_call, initialize_value,
    invalid_argument, require_output,
};

unsafe fn job_for_state(
    jobs: *const PpJobSet,
    index: u64,
    state: u32,
) -> Result<PpJob, postproject_core::Error> {
    // SAFETY: The exported callers require a non-null handle to be live.
    let jobs = unsafe { jobs.as_ref() }.ok_or_else(|| invalid_argument("jobs must not be null"))?;
    let index = usize::try_from(index).map_err(|_| invalid_argument("job index is too large"))?;
    jobs.get_for_state(index, state)
}

/// Reads claim detail, rejecting any other job state.
///
/// # Safety
///
/// `jobs` must be null or live. Outputs must be null or writable. Strings borrow
/// `jobs` and remain valid until it is released.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_set_get_claim(
    jobs: *const PpJobSet,
    index: u64,
    out_claim: *mut PpJobClaim,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and validated before writes.
    unsafe {
        initialize_value(out_claim, PpJobClaim::empty());
        ffi_call(out_error, || {
            require_output(out_claim, "out_claim")?;
            let job = job_for_state(jobs, index, PP_JOB_CLAIMED)?;
            out_claim.write(PpJobClaim::from_job(&job));
            Ok(())
        })
    }
}

/// Reads completion detail, rejecting any other job state.
///
/// # Safety
///
/// `jobs` must be null or live. Outputs must be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_set_get_completion(
    jobs: *const PpJobSet,
    index: u64,
    out_completion: *mut PpJobCompletion,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and validated before writes.
    unsafe {
        initialize_value(out_completion, PpJobCompletion::empty());
        ffi_call(out_error, || {
            require_output(out_completion, "out_completion")?;
            let job = job_for_state(jobs, index, PP_JOB_SUCCEEDED)?;
            out_completion.write(PpJobCompletion {
                activity_id: job.completion_activity_id,
                representation_id: job.completion_representation_id,
            });
            Ok(())
        })
    }
}

/// Reads a failed-job diagnostic, rejecting any other state.
///
/// # Safety
///
/// `jobs` must be null or live. Outputs must be null or writable. The returned
/// string borrows `jobs` until it is released.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_set_get_failure(
    jobs: *const PpJobSet,
    index: u64,
    out_diagnostic: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and validated before writes.
    unsafe {
        initialize_value(out_diagnostic, std::ptr::null());
        ffi_call(out_error, || {
            require_output(out_diagnostic, "out_diagnostic")?;
            let job = job_for_state(jobs, index, PP_JOB_FAILED)?;
            out_diagnostic.write(job.failure_diagnostic);
            Ok(())
        })
    }
}
