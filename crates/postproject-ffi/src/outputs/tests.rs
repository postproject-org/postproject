use super::*;
use crate::{
    PpCommitReceipt, PpProduction, SqliteProduction, begin_transaction_handle, lock_production,
    production_handle,
};
use std::{ffi::CString, ptr};

fn recipe() -> PpActivityRecipe {
    PpActivityRecipe {
        kind: c"example:render".as_ptr(),
        inputs: ptr::null(),
        input_count: 0,
        started_at_unix_micros: ptr::null(),
        finished_at_unix_micros: ptr::null(),
        tool_name: c"Actual fixture renderer".as_ptr(),
        tool_version: ptr::null(),
        tool_uri: ptr::null(),
        agent_name: ptr::null(),
        agent_identifier_scheme: ptr::null(),
        agent_identifier_value: ptr::null(),
        agent_identifier_qualifier: ptr::null(),
        parameters: ptr::null(),
        parameter_count: 0,
    }
}

fn source(path: &CString) -> PpOutputSource {
    PpOutputSource {
        asset_id: ptr::null(),
        kind: crate::PP_REPRESENTATION_ORIGINAL,
        path: path.as_ptr(),
        source: ptr::null(),
        role: ptr::null(),
        display_name: ptr::null(),
    }
}

fn store(path: &std::path::Path) -> PpProduction {
    production_handle(SqliteProduction::create(path, None).unwrap())
}

#[test]
fn late_media_error_clears_all_ids_and_stages_no_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("render.dat");
    std::fs::write(&file, b"already rendered").unwrap();
    let present = CString::new(file.to_str().unwrap()).unwrap();
    let absent = CString::new(directory.path().join("missing.dat").to_str().unwrap()).unwrap();
    let production = store(&directory.path().join("rollback.pproj"));
    let edit = begin_transaction_handle(&production.state, None, None).unwrap();
    let sources = [source(&present), source(&absent)];
    let mut identities = [PpOutputIdentity {
        asset_id: PpAssetId { bytes: [9; 16] },
        representation_id: PpRepresentationId { bytes: [9; 16] },
    }; 2];
    let mut activity = PpActivityId { bytes: [9; 16] };
    let recipe = recipe();
    let mut error = ptr::null_mut();
    // SAFETY: Borrowed strings, source arrays and outputs are live for this call.
    unsafe {
        assert_ne!(
            pp_transaction_register_outputs(
                edit,
                sources.as_ptr(),
                2,
                &raw const recipe,
                identities.as_mut_ptr(),
                &raw mut activity,
                &raw mut error
            ),
            crate::PP_OK
        );
        assert_eq!(activity.bytes, [0; 16]);
        assert!(
            identities
                .iter()
                .all(|id| id.asset_id.bytes == [0; 16] && id.representation_id.bytes == [0; 16])
        );
        assert!((*edit).mutations.is_empty());
        crate::pp_error_release(error);
        let mut receipt: PpCommitReceipt = std::mem::zeroed();
        assert_eq!(
            crate::pp_transaction_commit(edit, &raw mut receipt, ptr::null_mut()),
            crate::PP_OK
        );
        assert_eq!(receipt.outcome, 0);
        crate::pp_transaction_release(edit);
    }
    assert_eq!(
        lock_production(&production.state).assets().unwrap(),
        [] as [postproject_core::Asset; 0]
    );
}

#[test]
fn parameters_are_copied_and_attached_to_the_actual_activity() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("render.dat");
    std::fs::write(&file, b"generated pixels").unwrap();
    let path = CString::new(file.to_str().unwrap()).unwrap();
    let production = store(&directory.path().join("parameters.pproj"));
    let edit = begin_transaction_handle(&production.state, None, None).unwrap();
    let mut input = ptr::null_mut();
    let mut activity = PpActivityId { bytes: [0; 16] };
    let mut identity = PpOutputIdentity {
        asset_id: PpAssetId { bytes: [0; 16] },
        representation_id: PpRepresentationId { bytes: [0; 16] },
    };
    // SAFETY: Handles, strings and writable local outputs obey the ABI contract.
    unsafe {
        assert_eq!(
            crate::metadata_input::pp_metadata_input_create_i64(
                24,
                &raw mut input,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        let parameter = PpActivityParameter {
            vocabulary: c"example:recipe".as_ptr(),
            property: c"setting".as_ptr(),
            value: input,
        };
        let source = source(&path);
        let mut recipe = recipe();
        recipe.parameters = &raw const parameter;
        recipe.parameter_count = 1;
        assert_eq!(
            pp_transaction_register_outputs(
                edit,
                &raw const source,
                1,
                &raw const recipe,
                &raw mut identity,
                &raw mut activity,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        crate::metadata_input::pp_metadata_input_release(input);
        assert_eq!(
            lock_production(&production.state).activities().unwrap(),
            [] as [postproject_core::Activity; 0]
        );
        let mut receipt: PpCommitReceipt = std::mem::zeroed();
        assert_eq!(
            crate::pp_transaction_commit(edit, &raw mut receipt, ptr::null_mut()),
            crate::PP_OK
        );
        crate::pp_transaction_release(edit);
    }
    let store = lock_production(&production.state);
    let activity = postproject_core::ActivityId::from_bytes(activity.bytes);
    let assertions = store.metadata(ObjectRef::Activity(activity)).unwrap();
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].value().as_i64(), Some(24));
    assert_eq!(store.activities().unwrap()[0].id(), activity);
    let representation =
        postproject_core::RepresentationId::from_bytes(identity.representation_id.bytes);
    assert!(store.dependency_set(representation).unwrap().is_none());
}

#[test]
fn oversized_counts_do_not_read_array_pointers() {
    let mut activity = PpActivityId { bytes: [9; 16] };
    // SAFETY: Oversized counts reject before the deliberately dangling arrays.
    unsafe {
        assert_eq!(
            pp_transaction_register_outputs(
                ptr::null_mut(),
                ptr::dangling(),
                u64::MAX,
                ptr::null(),
                ptr::dangling_mut(),
                &raw mut activity,
                ptr::null_mut()
            ),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
    }
    assert_eq!(activity.bytes, [0; 16]);
}
