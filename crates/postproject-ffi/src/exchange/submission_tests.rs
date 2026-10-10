//! Public native delivery and private binding recovery after durable release.

use std::{ffi::CStr, ptr};

use postproject_core::{Job, JobId, JobKind, RepresentationKind, RequestedJobOutput};

use super::{outcome::*, submission::*, *};
use crate::{
    PpJobId, PpProduction, SqliteProduction, begin_transaction_handle,
    job_leases::{api::*, transitions::*},
    production_handle,
};

fn authority() -> (tempfile::TempDir, PpProduction, PpJobId) {
    let directory = tempfile::tempdir().unwrap();
    let media = directory.path().join("source.mov");
    std::fs::write(&media, b"prepared media").unwrap();
    let source = postproject_media::prepare_original_media(&media, None, None).unwrap();
    let job = Job::new(
        JobId::new(),
        JobKind::new("example:publish").unwrap(),
        vec![],
        RequestedJobOutput::new(source.asset().id(), RepresentationKind::Derived, None).unwrap(),
    )
    .unwrap();
    let mut store =
        SqliteProduction::create(directory.path().join("authority.pproj"), None).unwrap();
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

fn prepare(edit: *mut crate::PpTransaction, request: u8) -> *mut PpProposal {
    let mut proposal = ptr::null_mut();
    // SAFETY: Tests own a live serialized edit and writable output.
    unsafe {
        assert_eq!(
            pp_transaction_prepare_proposal(
                edit,
                PpClientId { bytes: [1; 16] },
                PpRequestId {
                    bytes: [request; 16]
                },
                ptr::null(),
                &raw mut proposal,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        crate::pp_transaction_release(edit);
    }
    proposal
}

fn submit(
    production: &PpProduction,
    proposal: *const PpProposal,
    leases: &[*const crate::PpJobLease],
) -> *mut PpSubmission {
    let mut result = ptr::null_mut();
    // SAFETY: Test owns each serialized handle/array through the complete call.
    unsafe {
        assert_eq!(
            pp_production_submit_proposal(
                production,
                proposal,
                leases.as_ptr(),
                u64::try_from(leases.len()).unwrap(),
                ptr::null(),
                0,
                &raw mut result,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
    }
    result
}

fn document(outcome: *const PpOutcome) -> Vec<u8> {
    let mut bytes = ptr::null();
    let mut length = 0;
    // SAFETY: Result stays live while copied bytes are borrowed.
    unsafe {
        assert_eq!(
            pp_outcome_document(outcome, &raw mut bytes, &raw mut length, ptr::null_mut()),
            crate::PP_OK
        );
        std::slice::from_raw_parts(bytes, usize::try_from(length).unwrap()).to_vec()
    }
}

fn fresh_claim(production: &PpProduction, job: PpJobId) -> *mut crate::PpJobLease {
    // SAFETY: Fixture owns all native handles exclusively, with writable outputs.
    unsafe {
        let edit = begin_transaction_handle(&production.state, None, None).unwrap();
        let mut pending = ptr::null_mut();
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
                &raw mut pending,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        let proposal = prepare(edit, 2);
        assert_eq!(
            (*pending).shared.state().unwrap(),
            postproject_core::JobLeaseState::Closed
        );
        pp_job_lease_free(pending);
        let delivery = submit(production, proposal, &[]);
        assert_eq!(pp_submission_lease_count(delivery), 1);
        let mut lease = ptr::null_mut();
        assert_eq!(
            pp_submission_take_lease(delivery, 0, &raw mut lease, ptr::null_mut()),
            crate::PP_OK
        );
        let mut repeated = ptr::dangling_mut();
        assert_eq!(
            pp_submission_take_lease(delivery, 0, &raw mut repeated, ptr::null_mut()),
            crate::PP_ERROR_NOT_FOUND
        );
        assert!(repeated.is_null());
        let duplicate = submit(production, proposal, &[]);
        assert_eq!(pp_submission_lease_count(duplicate), 0);
        let mut outcome = ptr::null_mut();
        assert_eq!(
            pp_submission_outcome(delivery, &raw mut outcome, ptr::null_mut()),
            crate::PP_OK
        );
        let mut token = ptr::null_mut();
        assert_eq!(
            pp_job_lease_export_token(lease, &raw mut token, ptr::null_mut()),
            crate::PP_OK
        );
        let secret = CStr::from_ptr(token)
            .to_str()
            .unwrap()
            .rsplit(':')
            .next()
            .unwrap()
            .as_bytes();
        assert!(
            !document(outcome)
                .windows(secret.len())
                .any(|bytes| bytes == secret)
        );
        assert!(
            !(*proposal)
                .bytes
                .windows(secret.len())
                .any(|bytes| bytes == secret)
        );
        crate::pp_string_release(token);
        pp_outcome_release(outcome);
        pp_submission_release(delivery);
        pp_submission_release(duplicate);
        pp_proposal_release(proposal);
        lease
    }
}

#[test]
fn private_native_handle_recovery_precedes_current_ownership_checks() {
    let (_directory, production, job) = authority();
    let lease = fresh_claim(&production, job);
    // SAFETY: All pointers remain live and serialized; every allocation freed once.
    unsafe {
        let edit = begin_transaction_handle(&production.state, None, None).unwrap();
        assert_eq!(
            pp_transaction_renew_job_lease(edit, lease, 60_000_000, ptr::null_mut()),
            crate::PP_OK
        );
        let proposal = prepare(edit, 3);
        let delivery = submit(&production, proposal, &[lease]);
        let mut original = ptr::null_mut();
        assert_eq!(
            pp_submission_outcome(delivery, &raw mut original, ptr::null_mut()),
            crate::PP_OK
        );
        pp_submission_release(delivery);
        let edit = begin_transaction_handle(&production.state, None, None).unwrap();
        assert_eq!(
            pp_transaction_release_job_lease(edit, lease, ptr::null_mut()),
            crate::PP_OK
        );
        assert_eq!(
            crate::pp_transaction_commit(edit, ptr::null_mut()),
            crate::PP_OK
        );
        crate::pp_transaction_release(edit);
        assert_eq!(
            (*lease).shared.state().unwrap(),
            postproject_core::JobLeaseState::Closed
        );
        let recovered = submit(&production, proposal, &[lease]);
        assert_eq!(pp_submission_lease_count(recovered), 0);
        let mut outcome = ptr::null_mut();
        assert_eq!(
            pp_submission_outcome(recovered, &raw mut outcome, ptr::null_mut()),
            crate::PP_OK
        );
        assert_eq!(document(outcome), document(original));
        pp_outcome_release(outcome);
        pp_submission_release(recovered);
        let mut identity = PpRequestIdentity::empty();
        assert_eq!(
            pp_proposal_identity(proposal, &raw mut identity, ptr::null_mut()),
            crate::PP_OK
        );
        assert_eq!(
            pp_production_submission_outcome(
                &raw const production,
                &raw const identity,
                &raw mut outcome,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        assert_eq!(document(outcome), document(original));
        pp_outcome_release(outcome);
        let mut rejected = ptr::dangling_mut();
        assert_eq!(
            pp_production_submit_proposal(
                &raw const production,
                proposal,
                ptr::null(),
                0,
                ptr::null(),
                0,
                &raw mut rejected,
                ptr::null_mut()
            ),
            16
        );
        assert!(rejected.is_null());
        let duplicates = [lease.cast_const(), lease.cast_const()];
        assert_eq!(
            pp_production_submit_proposal(
                &raw const production,
                proposal,
                duplicates.as_ptr(),
                2,
                ptr::null(),
                0,
                &raw mut rejected,
                ptr::null_mut()
            ),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
        assert!(rejected.is_null());
        assert_eq!(
            lock_production(&production.state)
                .latest_revision()
                .unwrap()
                .unwrap()
                .sequence(),
            4
        );
        pp_outcome_release(original);
        pp_proposal_release(proposal);
        pp_job_lease_free(lease);
    }
}
