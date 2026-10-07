//! Lease-only worker mutations; the storage authority validates at commit.

use postproject_core::{ActivityId, RepresentationId, validate_job_lease_duration};
use std::{ffi::c_char, sync::Arc, time::Duration};

use super::{LeaseMutation, PpJobLease};
use crate::{
    JobFailure, PpActivityId, PpError, PpRepresentationId, PpTransaction, StagedMutation, ffi_call,
    invalid_argument, required_utf8,
};

/// Stages renewal through an owning lease and checked microsecond duration.
///
/// # Safety
/// Transaction is live/exclusively accessed, lease live, error null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_renew_job_lease(
    transaction: *mut PpTransaction,
    lease: *const PpJobLease,
    duration_micros: u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Caller supplies valid live handles and output slots.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            let lease = lease
                .as_ref()
                .ok_or_else(|| invalid_argument("lease must not be null"))?;
            lease.shared.validate_edit(transaction)?;
            let duration = Duration::from_micros(duration_micros);
            validate_job_lease_duration(duration)?;
            transaction
                .mutations
                .push(StagedMutation::Lease(LeaseMutation::Renew(
                    Arc::clone(&lease.shared),
                    duration,
                )));
            Ok(())
        })
    }
}

/// Stages explicit release of a current, unexpired lease.
///
/// # Safety
/// Transaction is live/exclusively accessed, lease live, error null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_release_job_lease(
    transaction: *mut PpTransaction,
    lease: *const PpJobLease,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Caller supplies valid live handles and output slots.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            let lease = lease
                .as_ref()
                .ok_or_else(|| invalid_argument("lease must not be null"))?;
            lease.shared.validate_edit(transaction)?;
            transaction
                .mutations
                .push(StagedMutation::Lease(LeaseMutation::Release(Arc::clone(
                    &lease.shared,
                ))));
            Ok(())
        })
    }
}

/// Stages failure through a current, unexpired lease.
///
/// # Safety
/// Transaction is live/exclusively accessed, lease live, diagnostic terminated
/// UTF-8, and error null or writable. Inputs are copied during this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_fail_job_lease(
    transaction: *mut PpTransaction,
    lease: *const PpJobLease,
    diagnostic: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Caller supplies valid live handles and a readable diagnostic.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            let lease = lease
                .as_ref()
                .ok_or_else(|| invalid_argument("lease must not be null"))?;
            lease.shared.validate_edit(transaction)?;
            let failure = JobFailure::new(required_utf8(diagnostic, "diagnostic")?)?;
            transaction
                .mutations
                .push(StagedMutation::Lease(LeaseMutation::Fail(
                    Arc::clone(&lease.shared),
                    failure,
                )));
            Ok(())
        })
    }
}

/// Binds staged output/activity into lease completion and the edit's commit.
///
/// # Safety
/// Transaction is live/exclusively accessed, lease live, error null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_complete_job_lease(
    transaction: *mut PpTransaction,
    lease: *const PpJobLease,
    output_representation_id: PpRepresentationId,
    activity_id: PpActivityId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Required handles are checked before access; IDs are values.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            let lease = lease
                .as_ref()
                .ok_or_else(|| invalid_argument("lease must not be null"))?;
            lease.shared.validate_edit(transaction)?;
            let output_id = RepresentationId::from_bytes(output_representation_id.bytes);
            let activity_id = ActivityId::from_bytes(activity_id.bytes);
            let output_index = transaction.mutations.iter().position(|mutation|
                matches!(mutation, StagedMutation::Representation(output) if output.representation().id() == output_id))
                .ok_or_else(|| invalid_argument("output representation is not staged in this transaction"))?;
            let activity_index = transaction.mutations.iter().position(|mutation|
                matches!(mutation, StagedMutation::Activity(activity) if activity.id() == activity_id))
                .ok_or_else(|| invalid_argument("activity is not staged in this transaction"))?;
            if output_index >= activity_index {
                return Err(invalid_argument(
                    "job output representation must be staged before its activity",
                ));
            }
            let StagedMutation::Activity(activity) = transaction.mutations.remove(activity_index)
            else {
                unreachable!("selected activity variant");
            };
            let StagedMutation::Representation(output) = transaction.mutations.remove(output_index)
            else {
                unreachable!("selected output variant");
            };
            transaction.mutations.insert(
                output_index,
                StagedMutation::Lease(LeaseMutation::Complete {
                    lease: Arc::clone(&lease.shared),
                    output,
                    activity: Box::new(activity),
                }),
            );
            Ok(())
        })
    }
}
