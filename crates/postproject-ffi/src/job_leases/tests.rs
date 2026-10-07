//! Direct native ownership and boundary regressions.

use std::{ffi::CStr, ptr};

use postproject_core::{Job, JobKind, RepresentationKind, RequestedJobOutput};
use tempfile::{TempDir, tempdir};

use super::{api::*, transitions::*};
use crate::{
    PpJobId, PpJobLease, PpProduction, SqliteProduction, begin_transaction_handle,
    pp_transaction_release, production_handle,
};

fn production() -> (TempDir, PpProduction, PpJobId) {
    let directory = tempdir().unwrap();
    let media = directory.path().join("source.mov");
    std::fs::write(&media, b"media").unwrap();
    let source = postproject_media::prepare_original_media(media.as_path(), None, None).unwrap();
    let job = Job::new(
        postproject_core::JobId::new(),
        JobKind::new("example:publish").unwrap(),
        vec![],
        RequestedJobOutput::new(source.asset().id(), RepresentationKind::Derived, None).unwrap(),
    )
    .unwrap();
    let mut store =
        SqliteProduction::create(directory.path().join("production.pproj"), None).unwrap();
    let mut edit = store.begin_transaction().unwrap();
    edit.import_original(&source).unwrap();
    edit.request_job(&job).unwrap();
    edit.commit().unwrap();
    drop(edit);
    (
        directory,
        production_handle(store),
        PpJobId {
            bytes: job.id().into_bytes(),
        },
    )
}

#[test]
fn native_pending_claims_close_on_discard_and_activate_only_after_commit() {
    let (_directory, mut production, job) = production();
    // SAFETY: All handles come from this fixture and are exclusively accessed;
    // outputs are writable and library allocations are freed exactly once.
    unsafe {
        let edit = begin_transaction_handle(&production.state, None, None).unwrap();
        let mut lease = ptr::null_mut();
        assert_eq!(
            pp_transaction_claim_job_lease(
                edit,
                job,
                c"worker".as_ptr(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                60_000_000,
                &raw mut lease,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        let mut token = ptr::null_mut();
        assert_ne!(
            pp_job_lease_export_token(lease, &raw mut token, ptr::null_mut()),
            crate::PP_OK
        );
        assert!(token.is_null());
        pp_transaction_release(edit);
        assert_eq!(
            (*lease).shared.state().unwrap(),
            postproject_core::JobLeaseState::Closed
        );
        pp_job_lease_free(lease);

        let edit = begin_transaction_handle(&production.state, None, None).unwrap();
        assert_eq!(
            pp_transaction_claim_job_lease(
                edit,
                job,
                c"worker".as_ptr(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                60_000_000,
                &raw mut lease,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        assert_eq!(
            crate::pp_transaction_commit(edit, ptr::null_mut()),
            crate::PP_OK
        );
        pp_transaction_release(edit);
        assert_eq!(
            pp_job_lease_export_token(lease, &raw mut token, ptr::null_mut()),
            crate::PP_OK
        );
        assert_eq!(CStr::from_ptr(token).to_bytes().len(), 115);
        let mut imported: *mut PpJobLease = ptr::null_mut();
        assert_eq!(
            pp_production_import_job_lease(
                &raw mut production,
                token.cast(),
                115,
                &raw mut imported,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        crate::pp_string_release(token);
        let edit = begin_transaction_handle(&production.state, None, None).unwrap();
        assert_eq!(
            pp_transaction_release_job_lease(edit, lease, ptr::null_mut()),
            crate::PP_OK
        );
        assert_eq!(
            crate::pp_transaction_commit(edit, ptr::null_mut()),
            crate::PP_OK
        );
        pp_transaction_release(edit);
        assert_eq!(
            (*lease).shared.state().unwrap(),
            postproject_core::JobLeaseState::Closed
        );
        let edit = begin_transaction_handle(&production.state, None, None).unwrap();
        assert_eq!(
            pp_transaction_fail_job_lease(edit, imported, c"late".as_ptr(), ptr::null_mut()),
            crate::PP_OK
        );
        assert_eq!(
            crate::pp_transaction_commit(edit, ptr::null_mut()),
            crate::PP_ERROR_CONFLICT
        );
        pp_transaction_release(edit);
        pp_job_lease_free(imported);
        pp_job_lease_free(lease);
        pp_job_lease_free(ptr::null_mut());
    }
}

#[test]
fn native_rejects_invalid_durations_and_token_size_before_reading_input() {
    let (_directory, mut production, job) = production();
    // SAFETY: Oversized/null token input is rejected before borrowing bytes;
    // handles are live and outputs are writable, including every failure case.
    unsafe {
        let edit = begin_transaction_handle(&production.state, None, None).unwrap();
        let mut lease = ptr::null_mut();
        for duration in [0, 86_400_000_001, u64::MAX] {
            assert_eq!(
                pp_transaction_claim_job_lease(
                    edit,
                    job,
                    c"worker".as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    duration,
                    &raw mut lease,
                    ptr::null_mut()
                ),
                crate::PP_ERROR_INVALID_ARGUMENT
            );
            assert!(lease.is_null());
        }
        assert_eq!(
            pp_production_import_job_lease(
                &raw mut production,
                ptr::null(),
                u64::MAX,
                &raw mut lease,
                ptr::null_mut()
            ),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
        assert!(lease.is_null());
        assert_eq!(
            pp_production_import_job_lease(
                &raw mut production,
                ptr::null(),
                115,
                &raw mut lease,
                ptr::null_mut()
            ),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            crate::pp_transaction_commit(edit, ptr::null_mut()),
            crate::PP_OK
        );
        pp_transaction_release(edit);
    }
}
