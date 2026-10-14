//! Selection outputs borrow the set; failures never lend a partial result.

use crate::*;
use postproject_core::{
    Confidence, ContentStructure, ProductionId, ResolutionCandidate, ResourceResolution,
    TransactionState,
};

#[test]
fn verified_selection_checks_capacity_and_clears_failed_outputs() {
    let resource = ResourceId::new();
    let representation = RepresentationId::new();
    let base = DecisionBase::new(ProductionId::new(), None, 0).unwrap();
    let candidate = ResolutionCandidate::new(
        "file:///moved.mov",
        Confidence::CERTAIN,
        vec![ResolutionEvidence::new(EvidenceKind::FullHashMatch, None)],
    )
    .unwrap()
    .with_media_root("rushes")
    .unwrap();
    let resolution = RepresentationResolution::aggregate(
        representation,
        &ContentStructure::single_resource(resource),
        vec![
            ResourceResolution::new(
                resource,
                ResourceResolutionState::ResolvedExact,
                vec![candidate],
                Vec::new(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let set =
        PpResolutionSet::new_with_base(vec![(AssetId::new(), resolution)], Some(base)).unwrap();
    // SAFETY: Zero is a valid empty representation for this pointer/integer struct.
    let mut selected: PpRelinkSelection = unsafe { std::mem::zeroed() };
    let mut location = PpRelinkLocation {
        resource_id: PpResourceId { bytes: [9; 16] },
        resource_index: 0,
        uri: c"sentinel".as_ptr(),
        media_root: ptr::null(),
        has_sequence_naming: 0,
        sequence_naming: PpSequenceNaming {
            prefix: ptr::null(),
            suffix: ptr::null(),
            padding: 0,
        },
    };
    let mut error = ptr::null_mut();
    // SAFETY: All handles and outputs are live and writable for these calls.
    unsafe {
        assert_eq!(
            pp_resolution_set_select_verified(
                &raw const set,
                0,
                &raw mut location,
                1,
                &raw mut selected,
                &raw mut error
            ),
            PP_OK
        );
        assert_eq!(selected.location_count, 1);
        assert_eq!(DecisionBase::try_from(selected.base).unwrap(), base);
        assert_eq!(
            selected.representation_id.bytes,
            representation.into_bytes()
        );
        assert_eq!(location.resource_id.bytes, resource.into_bytes());
        assert_eq!(
            CStr::from_ptr(location.uri).to_str().unwrap(),
            "file:///moved.mov"
        );
        assert_eq!(
            CStr::from_ptr(location.media_root).to_str().unwrap(),
            "rushes"
        );
        location.uri = c"sentinel".as_ptr();
        assert_eq!(
            pp_resolution_set_select_verified(
                &raw const set,
                0,
                &raw mut location,
                0,
                &raw mut selected,
                &raw mut error
            ),
            PP_ERROR_INVALID_ARGUMENT
        );
        pp_error_release(error);
        assert_eq!(selected.location_count, 0);
        assert!(selected.locations.is_null());
        assert_eq!(CStr::from_ptr(location.uri).to_str().unwrap(), "sentinel");
        assert_eq!(
            pp_resolution_set_select_verified(
                ptr::null(),
                0,
                &raw mut location,
                1,
                &raw mut selected,
                &raw mut error
            ),
            PP_ERROR_INVALID_ARGUMENT
        );
        pp_error_release(error);
        assert_eq!(selected.location_count, 0);
    }
}

#[test]
fn confirmation_rejects_oversized_arrays_without_reading_them() {
    let directory = tempfile::tempdir().unwrap();
    let production = production_handle(
        SqliteProduction::create(directory.path().join("bounded.pproj"), None).unwrap(),
    );
    let base = lock_production(&production.state)
        .read_session()
        .unwrap()
        .decision_base();
    let transaction = begin_transaction_handle(&production.state, None, Some(base)).unwrap();
    let selection = PpRelinkSelection {
        base: base.into(),
        representation_id: PpRepresentationId { bytes: [9; 16] },
        locations: ptr::dangling(),
        location_count: 100_001,
    };
    let mut error = ptr::null_mut();
    // SAFETY: Oversized count is rejected before the deliberately unreadable array.
    unsafe {
        assert_eq!(
            pp_transaction_confirm_selection(transaction, &raw const selection, &raw mut error),
            PP_ERROR_INVALID_ARGUMENT
        );
        pp_error_release(error);
        assert!((*transaction).mutations.is_empty());
        assert_eq!((*transaction).lifecycle.state(), TransactionState::Open);
        pp_transaction_release(transaction);
    }
}
