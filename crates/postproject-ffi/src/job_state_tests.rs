//! Direct ABI regressions for checked job detail.

use crate::job_state::*;
use crate::{PpJobClaim, PpJobCompletion, PpJobSet};
use postproject_core::{AssetId, Job, JobId, JobKind, RepresentationKind, RequestedJobOutput};

#[test]
fn checked_state_accessors_reject_and_clear_wrong_state_missing_index_and_null() {
    let job = Job::new(
        JobId::new(),
        JobKind::new("example:work").expect("kind"),
        vec![],
        RequestedJobOutput::new(AssetId::new(), RepresentationKind::Proxy, None).expect("output"),
    )
    .expect("request");
    let jobs = PpJobSet::new(&[job]).expect("owned set");
    let cases = [
        (&raw const jobs, 0, crate::PP_ERROR_INVALID_ARGUMENT),
        (&raw const jobs, 1, crate::PP_ERROR_NOT_FOUND),
        (std::ptr::null(), 0, crate::PP_ERROR_INVALID_ARGUMENT),
    ];
    for (handle, index, expected) in cases {
        let mut claim = PpJobClaim::empty();
        claim.id.bytes = [7; 16];
        claim.expires_at_unix_micros = 99;
        claim.tool_name = c"marker".as_ptr();
        let mut completion = PpJobCompletion {
            activity_id: crate::PpUuid { bytes: [7; 16] },
            representation_id: crate::PpUuid { bytes: [7; 16] },
        };
        let mut diagnostic = c"marker".as_ptr();
        // SAFETY: Handles are null or live; each output is writable.
        unsafe {
            assert_eq!(
                pp_job_set_get_claim(handle, index, &raw mut claim, std::ptr::null_mut()),
                expected
            );
            assert_eq!(
                pp_job_set_get_completion(handle, index, &raw mut completion, std::ptr::null_mut()),
                expected
            );
            assert_eq!(
                pp_job_set_get_failure(handle, index, &raw mut diagnostic, std::ptr::null_mut()),
                expected
            );
        }
        assert_eq!(claim.id.bytes, [0; 16]);
        assert_eq!(claim.expires_at_unix_micros, 0);
        assert!(claim.tool_name.is_null());
        assert!(claim.tool_version.is_null());
        assert!(claim.tool_uri.is_null());
        assert!(claim.agent_name.is_null());
        assert!(claim.agent_identifier_scheme.is_null());
        assert!(claim.agent_identifier_value.is_null());
        assert!(claim.agent_identifier_qualifier.is_null());
        assert_eq!(completion.activity_id.bytes, [0; 16]);
        assert_eq!(completion.representation_id.bytes, [0; 16]);
        assert!(diagnostic.is_null());
    }
    // SAFETY: The live set is borrowed and null outputs are accepted errors.
    unsafe {
        assert_eq!(
            pp_job_set_get_claim(
                &raw const jobs,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            pp_job_set_get_completion(
                &raw const jobs,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            pp_job_set_get_failure(
                &raw const jobs,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
    }
}
