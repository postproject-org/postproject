//! C ABI change delivery: event-type-filtered revision pages and waiters.
//!
//! A waiter never touches its production handle after creation, and nothing
//! here calls back into foreign code (ADR 0027).

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, TryLockError},
    time::Duration,
};

use postproject_core::{
    Error, ErrorKind, RevisionEventFilter, RevisionEventType, RevisionWaitOutcome,
};
use postproject_storage_sqlite::{RevisionWaitCanceller, SqliteRevisionWaiter};

use crate::{
    PP_REVISION_ACTIVITY_CREATED, PP_REVISION_ACTIVITY_INPUT_ADDED,
    PP_REVISION_ACTIVITY_OUTPUT_ADDED, PP_REVISION_ASSET_IMPORTED,
    PP_REVISION_DEPENDENCY_SET_RECORDED, PP_REVISION_EXTERNAL_IDENTIFIER_ADDED,
    PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED, PP_REVISION_JOB_CANCELLED,
    PP_REVISION_JOB_CLAIM_RELEASED, PP_REVISION_JOB_CLAIM_RENEWED, PP_REVISION_JOB_CLAIMED,
    PP_REVISION_JOB_FAILED, PP_REVISION_JOB_REQUESTED, PP_REVISION_JOB_SUCCEEDED,
    PP_REVISION_LOCATOR_ADDED, PP_REVISION_LOCATOR_RETIRED, PP_REVISION_MEDIA_ROOT_ADDED,
    PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED, PP_REVISION_MEDIA_ROOT_REMOVED,
    PP_REVISION_METADATA_ADDED_OR_REPLACED, PP_REVISION_METADATA_REMOVED,
    PP_REVISION_REPRESENTATION_ADDED, PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED,
    PP_REVISION_REPRESENTATION_RESOURCE_ADDED, PP_REVISION_RESOURCE_ADDED,
    PP_REVISION_RESOURCE_FILE_FACTS_OBSERVED, PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED, PpError,
    PpProduction, PpRevisionSet, ffi_call, initialize_output, invalid_argument, lock_production,
    require_output,
};

const PP_REVISION_WAIT_REVISIONS: u32 = 1;
const PP_REVISION_WAIT_TIMED_OUT: u32 = 2;
const PP_REVISION_WAIT_CLOSED: u32 = 3;
const PP_REVISION_WAIT_CANCELLED: u32 = 4;

/// Maximum event-kind values accepted by one filtered revision page.
const MAX_FILTER_KINDS: u64 = 64;

/// Opaque revision waiter owned by the C caller.
pub struct PpRevisionWaiter {
    waiter: Mutex<SqliteRevisionWaiter>,
    canceller: RevisionWaitCanceller,
}

/// Loads revisions after `sequence` that contain an event of the given kinds.
///
/// # Safety
///
/// `production` must be live; `kinds` must point to `kind_count` readable
/// values; outputs must be writable; and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn pp_production_changes_since_filtered(
    production: *const PpProduction,
    sequence: u64,
    kinds: *const u32,
    kind_count: u64,
    limit: u32,
    out_revisions: *mut *mut PpRevisionSet,
    out_through_sequence: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and every pointer is checked before use;
    // the caller guarantees the kinds array covers `kind_count` values.
    unsafe {
        initialize_output(out_revisions);
        if !out_through_sequence.is_null() {
            out_through_sequence.write(0);
        }
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_revisions, "out_revisions")?;
            if out_through_sequence.is_null() {
                return Err(invalid_argument("out_through_sequence must not be null"));
            }
            if kinds.is_null() || kind_count == 0 || kind_count > MAX_FILTER_KINDS {
                return Err(invalid_argument(format!(
                    "kinds must name 1-{MAX_FILTER_KINDS} revision event kinds"
                )));
            }
            let count = usize::try_from(kind_count)
                .map_err(|_| invalid_argument("kind_count is not addressable"))?;
            let filter = RevisionEventFilter::new(
                std::slice::from_raw_parts(kinds, count)
                    .iter()
                    .map(|kind| revision_event_type(*kind))
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
            let page = lock_production(&production.state)
                .changes_since_filtered(sequence, &filter, limit)?;
            let revisions = PpRevisionSet::new(page.revisions())?;
            out_through_sequence.write(page.through_sequence());
            out_revisions.write(Box::into_raw(Box::new(revisions)));
            Ok(())
        })
    }
}

/// Creates a revision waiter with its own connection to the production file.
///
/// # Safety
///
/// `production` must be live, `out_waiter` writable, and `out_error` null or
/// writable. A successful waiter must be released exactly once with
/// [`pp_revision_waiter_release`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_waiter_create(
    production: *const PpProduction,
    out_waiter: *mut *mut PpRevisionWaiter,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and pointers checked before use.
    unsafe {
        initialize_output(out_waiter);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_waiter, "out_waiter")?;
            let waiter = lock_production(&production.state).revision_waiter()?;
            let canceller = waiter.canceller();
            out_waiter.write(Box::into_raw(Box::new(PpRevisionWaiter {
                waiter: Mutex::new(waiter),
                canceller,
            })));
            Ok(())
        })
    }
}

/// Waits for revisions after `after_sequence`, a timeout, close, or cancel.
///
/// # Safety
///
/// `waiter` must be live and not released during the call; outputs must be
/// writable; and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_waiter_wait(
    waiter: *mut PpRevisionWaiter,
    after_sequence: u64,
    limit: u32,
    timeout_millis: u32,
    out_result: *mut u32,
    out_revisions: *mut *mut PpRevisionSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and pointers checked before use. Only a
    // shared reference to the waiter is formed, so a concurrent cancel is sound.
    unsafe {
        initialize_output(out_revisions);
        if !out_result.is_null() {
            out_result.write(0);
        }
        ffi_call(out_error, || {
            let waiter = waiter
                .cast_const()
                .as_ref()
                .ok_or_else(|| invalid_argument("waiter must not be null"))?;
            if out_result.is_null() {
                return Err(invalid_argument("out_result must not be null"));
            }
            require_output(out_revisions, "out_revisions")?;
            let mut inner = match waiter.waiter.try_lock() {
                Ok(inner) => inner,
                Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
                Err(TryLockError::WouldBlock) => {
                    return Err(Error::new(
                        ErrorKind::Conflict,
                        "another thread is already waiting on this revision waiter",
                    ));
                }
            };
            let outcome = inner.wait_for_revisions(
                after_sequence,
                limit,
                Duration::from_millis(u64::from(timeout_millis)),
            )?;
            let (result, revisions) = match outcome {
                RevisionWaitOutcome::Revisions(revisions) => {
                    (PP_REVISION_WAIT_REVISIONS, PpRevisionSet::new(&revisions)?)
                }
                RevisionWaitOutcome::TimedOut => {
                    (PP_REVISION_WAIT_TIMED_OUT, PpRevisionSet::new(&[])?)
                }
                RevisionWaitOutcome::Closed => (PP_REVISION_WAIT_CLOSED, PpRevisionSet::new(&[])?),
                RevisionWaitOutcome::Cancelled => {
                    (PP_REVISION_WAIT_CANCELLED, PpRevisionSet::new(&[])?)
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::Unsupported,
                        "revision wait outcome is not supported by this ABI",
                    ));
                }
            };
            out_result.write(result);
            out_revisions.write(Box::into_raw(Box::new(revisions)));
            Ok(())
        })
    }
}

/// Cancels a waiter from any thread. Null is a no-op.
///
/// # Safety
///
/// A non-null `waiter` must be live for the duration of the call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_waiter_cancel(waiter: *mut PpRevisionWaiter) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Only a shared reference is formed; the caller keeps it live.
        if let Some(waiter) = unsafe { waiter.cast_const().as_ref() } {
            waiter.canceller.cancel();
        }
    }));
}

/// Releases a revision waiter. Null is a no-op.
///
/// # Safety
///
/// A non-null pointer must be live, released exactly once, and not used by
/// another thread during or after this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_waiter_release(waiter: *mut PpRevisionWaiter) {
    if waiter.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(waiter) });
    }));
}

fn revision_event_type(kind: u32) -> Result<RevisionEventType, Error> {
    Ok(match kind {
        PP_REVISION_ASSET_IMPORTED => RevisionEventType::AssetImported,
        PP_REVISION_REPRESENTATION_ADDED => RevisionEventType::RepresentationAdded,
        PP_REVISION_RESOURCE_ADDED => RevisionEventType::ResourceAdded,
        PP_REVISION_REPRESENTATION_RESOURCE_ADDED => RevisionEventType::RepresentationResourceAdded,
        PP_REVISION_LOCATOR_ADDED => RevisionEventType::LocatorAdded,
        PP_REVISION_MEDIA_ROOT_ADDED => RevisionEventType::MediaRootAdded,
        PP_REVISION_EXTERNAL_IDENTIFIER_ADDED => RevisionEventType::ExternalIdentifierAdded,
        PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED => RevisionEventType::ExternalIdentifierRemoved,
        PP_REVISION_METADATA_ADDED_OR_REPLACED => RevisionEventType::MetadataAddedOrReplaced,
        PP_REVISION_METADATA_REMOVED => RevisionEventType::MetadataRemoved,
        PP_REVISION_ACTIVITY_CREATED => RevisionEventType::ActivityCreated,
        PP_REVISION_ACTIVITY_INPUT_ADDED => RevisionEventType::ActivityInputAdded,
        PP_REVISION_ACTIVITY_OUTPUT_ADDED => RevisionEventType::ActivityOutputAdded,
        PP_REVISION_LOCATOR_RETIRED => RevisionEventType::LocatorRetired,
        PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED => RevisionEventType::MediaRootEnabledChanged,
        PP_REVISION_MEDIA_ROOT_REMOVED => RevisionEventType::MediaRootRemoved,
        PP_REVISION_RESOURCE_FILE_FACTS_OBSERVED => RevisionEventType::ResourceFileFactsObserved,
        PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED => RevisionEventType::ResourceFingerprintObserved,
        PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED => {
            RevisionEventType::RepresentationFingerprintObserved
        }
        PP_REVISION_DEPENDENCY_SET_RECORDED => RevisionEventType::DependencySetRecorded,
        PP_REVISION_JOB_REQUESTED => RevisionEventType::JobRequested,
        PP_REVISION_JOB_CLAIMED => RevisionEventType::JobClaimed,
        PP_REVISION_JOB_CLAIM_RENEWED => RevisionEventType::JobClaimRenewed,
        PP_REVISION_JOB_CLAIM_RELEASED => RevisionEventType::JobClaimReleased,
        PP_REVISION_JOB_SUCCEEDED => RevisionEventType::JobSucceeded,
        PP_REVISION_JOB_FAILED => RevisionEventType::JobFailed,
        PP_REVISION_JOB_CANCELLED => RevisionEventType::JobCancelled,
        _ => {
            return Err(invalid_argument(format!(
                "unknown revision event kind {kind}"
            )));
        }
    })
}

/// Closes every waiter created from a production when its handle is released.
pub(crate) fn close_production_waiters(production: &PpProduction) {
    lock_production(&production.state).close_revision_waiters();
}

#[cfg(test)]
mod tests {
    use std::{ptr, thread};

    use postproject_storage_sqlite::SqliteProduction;

    use super::*;
    use crate::{
        PP_ERROR_CONFLICT, PP_OK, pp_error_release, pp_revision_set_release, production_handle,
    };

    #[test]
    fn cancel_from_another_thread_releases_a_blocked_wait() {
        let directory = tempfile::tempdir().expect("create directory");
        let production = SqliteProduction::create(directory.path().join("waits.pproj"), None)
            .expect("create production");
        let production = production_handle(production);
        let mut waiter = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: The production is live and the outputs are local.
        assert_eq!(
            unsafe {
                pp_revision_waiter_create(&raw const production, &raw mut waiter, &raw mut error)
            },
            PP_OK
        );
        let address = waiter as usize;
        let blocked = thread::spawn(move || {
            let mut result = 0;
            let mut revisions = ptr::null_mut();
            let mut error = ptr::null_mut();
            // SAFETY: The waiter stays live until this thread is joined.
            let status = unsafe {
                pp_revision_waiter_wait(
                    address as *mut PpRevisionWaiter,
                    0,
                    1,
                    60_000,
                    &raw mut result,
                    &raw mut revisions,
                    &raw mut error,
                )
            };
            // SAFETY: Each owned output is released once.
            unsafe {
                pp_revision_set_release(revisions);
                pp_error_release(error);
            }
            (status, result)
        });
        // SAFETY: Cancellation is valid concurrently with a wait.
        unsafe { pp_revision_waiter_cancel(waiter) };
        assert_eq!(
            blocked.join().expect("join waiter"),
            (PP_OK, PP_REVISION_WAIT_CANCELLED)
        );
        // SAFETY: No other thread uses the waiter any more.
        unsafe { pp_revision_waiter_release(waiter) };
    }

    #[test]
    fn a_second_concurrent_wait_is_a_conflict() {
        let directory = tempfile::tempdir().expect("create directory");
        let production = SqliteProduction::create(directory.path().join("waits.pproj"), None)
            .expect("create production");
        let waiter = production.revision_waiter().expect("create waiter");
        let canceller = waiter.canceller();
        let handle = PpRevisionWaiter {
            waiter: Mutex::new(waiter),
            canceller,
        };
        let busy = handle.waiter.lock().expect("simulate an active wait");
        let mut result = 0;
        let mut revisions = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: The handle is live and the outputs are local.
        let status = unsafe {
            pp_revision_waiter_wait(
                (&raw const handle).cast_mut(),
                0,
                1,
                0,
                &raw mut result,
                &raw mut revisions,
                &raw mut error,
            )
        };
        drop(busy);
        assert_eq!(status, PP_ERROR_CONFLICT);
        assert!(revisions.is_null());
        // SAFETY: The error is owned and released once.
        unsafe { pp_error_release(error) };
    }
}
