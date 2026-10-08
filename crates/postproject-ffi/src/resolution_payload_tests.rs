//! Selected resolution states cannot expose another case's payload.

use crate::*;

#[test]
fn resolution_selection_checks_all_states_and_clears_wrong_payload_outputs() {
    for (state, count) in [
        (PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR, 1),
        (PP_RESOURCE_RESOLVED_EXACT, 1),
        (PP_RESOURCE_RESOLVED_PROBABLE, 1),
        (PP_RESOURCE_OFFLINE, 0),
        (PP_RESOURCE_AMBIGUOUS, 2),
        (PP_RESOURCE_RESOLUTION_ERROR, 0),
    ] {
        let resource = ResourceId::new();
        let resolutions = PpResolutionSet {
            representations: vec![AbiRepresentationResolution {
                asset_id: AssetId::new(),
                representation_id: RepresentationId::new(),
                availability: RepresentationAvailability::Online,
                resources: vec![AbiResolution {
                    resource_id: resource,
                    state,
                    candidates: (0..count)
                        .map(|_| AbiCandidate {
                            uri: sanitized_cstring("file:///example.mov"),
                            confidence: 10_000,
                            media_root: None,
                            sequence_naming: None,
                            evidence: Vec::new(),
                        })
                        .collect(),
                    evidence: Vec::new(),
                }],
                issues: Vec::new(),
            }],
        };
        let mut observed = 99;
        let mut error = ptr::null_mut();
        // SAFETY: The borrowed set remains live and every output is writable.
        assert_eq!(
            unsafe {
                pp_resolution_set_get_resource_state(
                    &raw const resolutions,
                    0,
                    0,
                    &raw mut observed,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(observed, state);
        for selected in 0..=7 {
            let mut id = PpResourceId { bytes: [1; 16] };
            let mut output_state = 99;
            let mut candidate_count = 99;
            let mut evidence_count = 99;
            // SAFETY: The set remains live; all outputs are writable for this call.
            let status = unsafe {
                pp_resolution_set_get_resource(
                    &raw const resolutions,
                    0,
                    0,
                    selected,
                    &raw mut id,
                    &raw mut output_state,
                    &raw mut candidate_count,
                    &raw mut evidence_count,
                    &raw mut error,
                )
            };
            if selected == state {
                assert_eq!(status, PP_OK);
                assert_eq!(id.bytes, resource.into_bytes());
                assert_eq!(output_state, state);
                assert_eq!(candidate_count, count);
                assert!(error.is_null());
            } else {
                assert_eq!(status, PP_ERROR_INVALID_ARGUMENT);
                assert_eq!(id.bytes, [0; 16]);
                assert_eq!((output_state, candidate_count, evidence_count), (0, 0, 0));
                assert!(!error.is_null());
                // SAFETY: Release the owned error once, after inspecting the outputs.
                unsafe { pp_error_release(error) };
                error = ptr::null_mut();
            }
        }
        // SAFETY: The set remains live; the bad index rejects before access.
        assert_eq!(
            unsafe {
                pp_resolution_set_get_resource_state(
                    &raw const resolutions,
                    0,
                    1,
                    &raw mut observed,
                    &raw mut error,
                )
            },
            PP_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(observed, 0);
        // SAFETY: The failed call returned one owned error.
        unsafe { pp_error_release(error) };
    }
}
