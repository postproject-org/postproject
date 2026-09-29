use std::{ffi::CString, os::raw::c_char, ptr};

use postproject_core::{
    AssetId, ContentStructure, ContentStructureKind, Error, ErrorKind, Locator,
    LocatorAvailability, LocatorId, QueryCursor, Representation, RepresentationId,
    RepresentationKind, Resource, ResourceId,
};
use postproject_storage_sqlite::SqliteProduction;

use crate::{
    PpError, PpProduction, PpSequenceNaming, PpUuid, exact_cstring, ffi_call,
    initialize_const_output, initialize_output, initialize_uuid, initialize_value, item_at,
    lock_production, query_page_request, require_output, required_utf8,
    sequence_naming::{AbiSequenceNaming, initialize_naming_output, write_naming_output},
};

const PP_REPRESENTATION_ORIGINAL: u32 = 1;
const PP_REPRESENTATION_PROXY: u32 = 2;
const PP_REPRESENTATION_OPTIMIZED: u32 = 3;
const PP_REPRESENTATION_DERIVED: u32 = 4;

const PP_CONTENT_SINGLE_RESOURCE: u32 = 1;
const PP_CONTENT_IMAGE_SEQUENCE: u32 = 2;
const PP_CONTENT_ORDERED_PARTS: u32 = 3;
const PP_CONTENT_PACKAGE: u32 = 4;

const PP_LOCATOR_UNKNOWN: u32 = 1;
const PP_LOCATOR_ONLINE: u32 = 2;
const PP_LOCATOR_OFFLINE: u32 = 3;

/// Opaque immutable representation result set owned by the C caller.
pub struct PpRepresentationSet {
    representations: Vec<AbiRepresentation>,
    next_cursor: Option<CString>,
}

struct AbiRepresentation {
    id: RepresentationId,
    asset_id: AssetId,
    kind: u32,
    structure_kind: u32,
    members: Vec<AbiMember>,
    sequence: Option<AbiSequence>,
    resources: Vec<AbiResource>,
    fingerprints: Vec<AbiFingerprint>,
}

struct AbiMember {
    resource_id: ResourceId,
    role: Option<CString>,
    required: bool,
}

struct AbiSequence {
    start: i64,
    end: i64,
    step: u32,
    rate_numerator: u32,
    rate_denominator: u32,
    missing_frames: Vec<i64>,
}

struct AbiResource {
    id: ResourceId,
    file_size: Option<u64>,
    modified_at: Option<i64>,
    locators: Vec<AbiLocator>,
    fingerprints: Vec<AbiFingerprint>,
}

struct AbiFingerprint {
    algorithm: CString,
    version: u16,
    value: Vec<u8>,
}

struct AbiLocator {
    id: LocatorId,
    uri: CString,
    availability: u32,
    last_seen: Option<i64>,
    sequence_naming: Option<AbiSequenceNaming>,
}

/// Loads the representations belonging to one asset in stable order.
///
/// # Safety
///
/// All input pointers must be live and readable. `out_representations` must be
/// writable. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_representations(
    production: *const PpProduction,
    asset_id: *const PpUuid,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointers are validated before use and outputs are initialized.
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let asset_id = asset_id
                .as_ref()
                .ok_or_else(|| invalid_argument("asset_id must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let asset_id = AssetId::from_bytes(asset_id.bytes);
            let inner = lock_production(&production.state);
            inner.asset(asset_id)?;
            let mut representations = Vec::new();
            for representation in inner.representations(asset_id)? {
                let mut resources = Vec::new();
                for resource in inner.resources(representation.id())? {
                    let locators = inner.locators(resource.id())?;
                    resources.push(AbiResource::new(&resource, locators)?);
                }
                representations.push(AbiRepresentation::new(&representation, resources)?);
            }
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet {
                representations,
                next_cursor: None,
            })));
            Ok(())
        })
    }
}

/// Queries one bounded page of representations belonging to an asset.
///
/// # Safety
///
/// Input handles and IDs must be live, `cursor` must be null or NUL-terminated
/// UTF-8, outputs must be writable, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_representations_page(
    production: *const PpProduction,
    asset_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let asset_id = asset_id
                .as_ref()
                .ok_or_else(|| invalid_argument("asset_id must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let page_request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let page =
                inner.representations_page(AssetId::from_bytes(asset_id.bytes), &page_request)?;
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet::new_page(
                &inner,
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

/// Reads one representation by identity as a one-element representation set.
///
/// # Safety
///
/// `production` and `representation_id` must be live, `out_representations`
/// must be writable, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_representation(
    production: *const PpProduction,
    representation_id: *const PpUuid,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let representation_id = representation_id
                .as_ref()
                .ok_or_else(|| invalid_argument("representation_id must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let inner = lock_production(&production.state);
            let representation =
                inner.representation(RepresentationId::from_bytes(representation_id.bytes))?;
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet::new_page(
                &inner,
                &[representation],
                None,
            )?)));
            Ok(())
        })
    }
}

/// Queries one bounded page of representations that use a resource.
///
/// # Safety
///
/// Pointer rules match [`pp_production_representations_page`]; `resource_id`
/// must be live.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_representations_using_resource(
    production: *const PpProduction,
    resource_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let resource_id = resource_id
                .as_ref()
                .ok_or_else(|| invalid_argument("resource_id must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let page_request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let page = inner.representations_using_resource(
                ResourceId::from_bytes(resource_id.bytes),
                &page_request,
            )?;
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet::new_page(
                &inner,
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

/// Queries representations with confirmed locator knowledge under a logical root.
///
/// # Safety
///
/// Pointer rules match [`pp_production_representations_page`]; `root_name` is
/// required NUL-terminated UTF-8.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_representations_under_media_root(
    production: *const PpProduction,
    root_name: *const c_char,
    limit: u32,
    cursor: *const c_char,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let root_name = required_utf8(root_name, "root_name")?;
            require_output(out_representations, "out_representations")?;
            let page_request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let page = inner.representations_under_media_root(root_name, &page_request)?;
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet::new_page(
                &inner,
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

/// Returns the borrowed next-page cursor, or null for the last page.
///
/// # Safety
///
/// `representations` must be null or a live representation set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_next_cursor(
    representations: *const PpRepresentationSet,
) -> *const c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        representations
            .as_ref()
            .map_or(ptr::null(), PpRepresentationSet::next_cursor)
    }))
    .unwrap_or(ptr::null())
}

/// Returns the number of representations. Null input returns zero.
///
/// # Safety
///
/// `representations` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_count(
    representations: *const PpRepresentationSet,
) -> u64 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { representations.as_ref() }.map_or(0, |set| {
            u64::try_from(set.representations.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Reads one representation summary.
///
/// # Safety
///
/// The set must be live and every output pointer must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_get(
    representations: *const PpRepresentationSet,
    index: u64,
    out_id: *mut PpUuid,
    out_asset_id: *mut PpUuid,
    out_kind: *mut u32,
    out_structure_kind: *mut u32,
    out_member_count: *mut u64,
    out_resource_count: *mut u64,
    out_fingerprint_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_uuid(out_id);
        initialize_uuid(out_asset_id);
        initialize_value(out_kind, 0);
        initialize_value(out_structure_kind, 0);
        initialize_value(out_member_count, 0);
        initialize_value(out_resource_count, 0);
        initialize_value(out_fingerprint_count, 0);
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            require_output(out_asset_id, "out_asset_id")?;
            require_output(out_kind, "out_kind")?;
            require_output(out_structure_kind, "out_structure_kind")?;
            require_output(out_member_count, "out_member_count")?;
            require_output(out_resource_count, "out_resource_count")?;
            require_output(out_fingerprint_count, "out_fingerprint_count")?;
            let set = representations
                .as_ref()
                .ok_or_else(|| invalid_argument("representations must not be null"))?;
            let representation = item_at(&set.representations, index, "representation")?;
            out_id.write(PpUuid {
                bytes: representation.id.into_bytes(),
            });
            out_asset_id.write(PpUuid {
                bytes: representation.asset_id.into_bytes(),
            });
            out_kind.write(representation.kind);
            out_structure_kind.write(representation.structure_kind);
            out_member_count.write(u64::try_from(representation.members.len()).unwrap_or(u64::MAX));
            out_resource_count
                .write(u64::try_from(representation.resources.len()).unwrap_or(u64::MAX));
            out_fingerprint_count
                .write(u64::try_from(representation.fingerprints.len()).unwrap_or(u64::MAX));
            Ok(())
        })
    }
}

/// Reads one structure-aware representation fingerprint.
///
/// Returned algorithm and value pointers borrow the result set.
///
/// # Safety
///
/// The set must be live and every output pointer must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_get_fingerprint(
    representations: *const PpRepresentationSet,
    representation_index: u64,
    fingerprint_index: u64,
    out_algorithm: *mut *const c_char,
    out_version: *mut u16,
    out_value: *mut *const u8,
    out_value_length: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_const_output(out_algorithm);
        initialize_value(out_version, 0);
        initialize_const_output(out_value);
        initialize_value(out_value_length, 0);
        ffi_call(out_error, || {
            require_output(out_algorithm, "out_algorithm")?;
            require_output(out_version, "out_version")?;
            require_output(out_value, "out_value")?;
            require_output(out_value_length, "out_value_length")?;
            let set = representations
                .as_ref()
                .ok_or_else(|| invalid_argument("representations must not be null"))?;
            let representation =
                item_at(&set.representations, representation_index, "representation")?;
            let fingerprint = item_at(
                &representation.fingerprints,
                fingerprint_index,
                "representation fingerprint",
            )?;
            write_fingerprint(
                fingerprint,
                out_algorithm,
                out_version,
                out_value,
                out_value_length,
            );
            Ok(())
        })
    }
}

/// Reads one structural member. A null role means the structure has no role.
///
/// # Safety
///
/// The set must be live and every output pointer must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_get_member(
    representations: *const PpRepresentationSet,
    representation_index: u64,
    member_index: u64,
    out_resource_id: *mut PpUuid,
    out_role: *mut *const c_char,
    out_required: *mut u8,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_uuid(out_resource_id);
        initialize_const_output(out_role);
        initialize_value(out_required, 0);
        ffi_call(out_error, || {
            require_output(out_resource_id, "out_resource_id")?;
            require_output(out_role, "out_role")?;
            require_output(out_required, "out_required")?;
            let set = representations
                .as_ref()
                .ok_or_else(|| invalid_argument("representations must not be null"))?;
            let representation =
                item_at(&set.representations, representation_index, "representation")?;
            let member = item_at(&representation.members, member_index, "member")?;
            out_resource_id.write(PpUuid {
                bytes: member.resource_id.into_bytes(),
            });
            out_role.write(
                member
                    .role
                    .as_ref()
                    .map_or(ptr::null(), |role| role.as_ptr()),
            );
            out_required.write(u8::from(member.required));
            Ok(())
        })
    }
}

/// Reads the compact image-sequence descriptor for one representation.
///
/// # Safety
///
/// The set must be live and every output pointer must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_get_sequence(
    representations: *const PpRepresentationSet,
    representation_index: u64,
    out_start: *mut i64,
    out_end: *mut i64,
    out_step: *mut u32,
    out_rate_numerator: *mut u32,
    out_rate_denominator: *mut u32,
    out_missing_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_start, 0);
        initialize_value(out_end, 0);
        initialize_value(out_step, 0);
        initialize_value(out_rate_numerator, 0);
        initialize_value(out_rate_denominator, 0);
        initialize_value(out_missing_count, 0);
        ffi_call(out_error, || {
            require_output(out_start, "out_start")?;
            require_output(out_end, "out_end")?;
            require_output(out_step, "out_step")?;
            require_output(out_rate_numerator, "out_rate_numerator")?;
            require_output(out_rate_denominator, "out_rate_denominator")?;
            require_output(out_missing_count, "out_missing_count")?;
            let set = representations
                .as_ref()
                .ok_or_else(|| invalid_argument("representations must not be null"))?;
            let representation =
                item_at(&set.representations, representation_index, "representation")?;
            let sequence = representation
                .sequence
                .as_ref()
                .ok_or_else(|| invalid_argument("representation is not an image sequence"))?;
            out_start.write(sequence.start);
            out_end.write(sequence.end);
            out_step.write(sequence.step);
            out_rate_numerator.write(sequence.rate_numerator);
            out_rate_denominator.write(sequence.rate_denominator);
            out_missing_count
                .write(u64::try_from(sequence.missing_frames.len()).unwrap_or(u64::MAX));
            Ok(())
        })
    }
}

/// Reads one known missing frame from an image-sequence descriptor.
///
/// # Safety
///
/// The set must be live and `out_frame` writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_get_sequence_missing_frame(
    representations: *const PpRepresentationSet,
    representation_index: u64,
    frame_index: u64,
    out_frame: *mut i64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_frame, 0);
        ffi_call(out_error, || {
            require_output(out_frame, "out_frame")?;
            let set = representations
                .as_ref()
                .ok_or_else(|| invalid_argument("representations must not be null"))?;
            let representation =
                item_at(&set.representations, representation_index, "representation")?;
            let sequence = representation
                .sequence
                .as_ref()
                .ok_or_else(|| invalid_argument("representation is not an image sequence"))?;
            out_frame.write(*item_at(
                &sequence.missing_frames,
                frame_index,
                "missing frame",
            )?);
            Ok(())
        })
    }
}

/// Reads one concrete resource and its stored file facts.
///
/// # Safety
///
/// The set must be live and every output pointer must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_get_resource(
    representations: *const PpRepresentationSet,
    representation_index: u64,
    resource_index: u64,
    out_id: *mut PpUuid,
    out_has_file_facts: *mut u8,
    out_file_size: *mut u64,
    out_has_modified_at: *mut u8,
    out_modified_at_unix_micros: *mut i64,
    out_locator_count: *mut u64,
    out_fingerprint_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_uuid(out_id);
        initialize_value(out_has_file_facts, 0);
        initialize_value(out_file_size, 0);
        initialize_value(out_has_modified_at, 0);
        initialize_value(out_modified_at_unix_micros, 0);
        initialize_value(out_locator_count, 0);
        initialize_value(out_fingerprint_count, 0);
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            require_output(out_has_file_facts, "out_has_file_facts")?;
            require_output(out_file_size, "out_file_size")?;
            require_output(out_has_modified_at, "out_has_modified_at")?;
            require_output(out_modified_at_unix_micros, "out_modified_at_unix_micros")?;
            require_output(out_locator_count, "out_locator_count")?;
            require_output(out_fingerprint_count, "out_fingerprint_count")?;
            let resource = resource_at(representations, representation_index, resource_index)?;
            out_id.write(PpUuid {
                bytes: resource.id.into_bytes(),
            });
            if let Some(size) = resource.file_size {
                out_has_file_facts.write(1);
                out_file_size.write(size);
            }
            if let Some(modified_at) = resource.modified_at {
                out_has_modified_at.write(1);
                out_modified_at_unix_micros.write(modified_at);
            }
            out_locator_count.write(u64::try_from(resource.locators.len()).unwrap_or(u64::MAX));
            out_fingerprint_count
                .write(u64::try_from(resource.fingerprints.len()).unwrap_or(u64::MAX));
            Ok(())
        })
    }
}

/// Reads one storage-resource fingerprint.
///
/// Returned algorithm and value pointers borrow the result set.
///
/// # Safety
///
/// The set must be live and every output pointer must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_get_resource_fingerprint(
    representations: *const PpRepresentationSet,
    representation_index: u64,
    resource_index: u64,
    fingerprint_index: u64,
    out_algorithm: *mut *const c_char,
    out_version: *mut u16,
    out_value: *mut *const u8,
    out_value_length: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_const_output(out_algorithm);
        initialize_value(out_version, 0);
        initialize_const_output(out_value);
        initialize_value(out_value_length, 0);
        ffi_call(out_error, || {
            require_output(out_algorithm, "out_algorithm")?;
            require_output(out_version, "out_version")?;
            require_output(out_value, "out_value")?;
            require_output(out_value_length, "out_value_length")?;
            let resource = resource_at(representations, representation_index, resource_index)?;
            let fingerprint = item_at(
                &resource.fingerprints,
                fingerprint_index,
                "resource fingerprint",
            )?;
            write_fingerprint(
                fingerprint,
                out_algorithm,
                out_version,
                out_value,
                out_value_length,
            );
            Ok(())
        })
    }
}

/// Reads one locator belonging to a representation resource.
///
/// # Safety
///
/// The set must be live and every output pointer must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_get_locator(
    representations: *const PpRepresentationSet,
    representation_index: u64,
    resource_index: u64,
    locator_index: u64,
    out_id: *mut PpUuid,
    out_uri: *mut *const c_char,
    out_availability: *mut u32,
    out_has_last_seen: *mut u8,
    out_last_seen_unix_micros: *mut i64,
    out_has_sequence_naming: *mut u8,
    out_sequence_naming: *mut PpSequenceNaming,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_uuid(out_id);
        initialize_const_output(out_uri);
        initialize_value(out_availability, 0);
        initialize_value(out_has_last_seen, 0);
        initialize_value(out_last_seen_unix_micros, 0);
        initialize_naming_output(out_has_sequence_naming, out_sequence_naming);
        ffi_call(out_error, || {
            require_output(out_has_sequence_naming, "out_has_sequence_naming")?;
            require_output(out_sequence_naming, "out_sequence_naming")?;
            require_output(out_id, "out_id")?;
            require_output(out_uri, "out_uri")?;
            require_output(out_availability, "out_availability")?;
            require_output(out_has_last_seen, "out_has_last_seen")?;
            require_output(out_last_seen_unix_micros, "out_last_seen_unix_micros")?;
            let resource = resource_at(representations, representation_index, resource_index)?;
            let locator = item_at(&resource.locators, locator_index, "locator")?;
            out_id.write(PpUuid {
                bytes: locator.id.into_bytes(),
            });
            out_uri.write(locator.uri.as_ptr());
            out_availability.write(locator.availability);
            if let Some(last_seen) = locator.last_seen {
                out_has_last_seen.write(1);
                out_last_seen_unix_micros.write(last_seen);
            }
            write_naming_output(
                locator.sequence_naming.as_ref(),
                out_has_sequence_naming,
                out_sequence_naming,
            );
            Ok(())
        })
    }
}

/// Releases a representation result set. Null is a no-op.
///
/// # Safety
///
/// A non-null pointer must be an unreleased result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_set_release(representations: *mut PpRepresentationSet) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !representations.is_null() {
            // SAFETY: Ownership is transferred back exactly once by contract.
            drop(unsafe { Box::from_raw(representations) });
        }
    }));
}

impl PpRepresentationSet {
    pub(crate) fn new_page(
        production: &SqliteProduction,
        values: &[Representation],
        next_cursor: Option<&QueryCursor>,
    ) -> Result<Self, Error> {
        let mut representations = Vec::with_capacity(values.len());
        for representation in values {
            let mut resources = Vec::new();
            for resource in production.resources(representation.id())? {
                let locators = production.locators(resource.id())?;
                resources.push(AbiResource::new(&resource, locators)?);
            }
            representations.push(AbiRepresentation::new(representation, resources)?);
        }
        Ok(Self {
            representations,
            next_cursor: next_cursor
                .map(|cursor| exact_cstring(cursor.as_str(), "representation query cursor"))
                .transpose()?,
        })
    }

    pub(crate) fn next_cursor(&self) -> *const c_char {
        self.next_cursor
            .as_ref()
            .map_or(ptr::null(), |cursor| cursor.as_ptr())
    }
}

impl AbiRepresentation {
    fn new(representation: &Representation, resources: Vec<AbiResource>) -> Result<Self, Error> {
        let structure = representation.content_structure();
        Ok(Self {
            id: representation.id(),
            asset_id: representation.asset_id(),
            kind: representation_kind(representation.kind()),
            structure_kind: content_structure_kind(structure.kind()),
            members: members(structure)?,
            sequence: sequence(structure),
            resources,
            fingerprints: representation
                .fingerprints()
                .iter()
                .map(|fingerprint| {
                    AbiFingerprint::new(
                        fingerprint.algorithm(),
                        fingerprint.version(),
                        fingerprint.value(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl AbiResource {
    fn new(resource: &Resource, locators: Vec<Locator>) -> Result<Self, Error> {
        let file_facts = resource.file_facts();
        Ok(Self {
            id: resource.id(),
            file_size: file_facts.map(postproject_core::FileFacts::size_bytes),
            modified_at: file_facts
                .and_then(postproject_core::FileFacts::modified_at)
                .map(postproject_core::Timestamp::as_unix_micros),
            locators: locators
                .into_iter()
                .map(AbiLocator::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            fingerprints: resource
                .fingerprints()
                .iter()
                .map(|fingerprint| {
                    AbiFingerprint::new(
                        fingerprint.algorithm(),
                        fingerprint.version(),
                        fingerprint.value(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl AbiFingerprint {
    fn new(algorithm: &str, version: u16, value: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            algorithm: exact_cstring(algorithm, "fingerprint algorithm")?,
            version,
            value: value.to_vec(),
        })
    }
}

impl TryFrom<Locator> for AbiLocator {
    type Error = Error;

    fn try_from(locator: Locator) -> Result<Self, Self::Error> {
        Ok(Self {
            id: locator.id(),
            uri: exact_cstring(locator.uri(), "locator URI")?,
            availability: locator_availability(locator.availability()),
            last_seen: locator
                .last_seen()
                .map(postproject_core::Timestamp::as_unix_micros),
            sequence_naming: AbiSequenceNaming::optional(locator.sequence_naming())?,
        })
    }
}

unsafe fn resource_at<'a>(
    representations: *const PpRepresentationSet,
    representation_index: u64,
    resource_index: u64,
) -> Result<&'a AbiResource, Error> {
    // SAFETY: The caller contract keeps the set alive for the returned borrow.
    let set = unsafe { representations.as_ref() }
        .ok_or_else(|| invalid_argument("representations must not be null"))?;
    let representation = item_at(&set.representations, representation_index, "representation")?;
    item_at(&representation.resources, resource_index, "resource")
}

unsafe fn write_fingerprint(
    fingerprint: &AbiFingerprint,
    out_algorithm: *mut *const c_char,
    out_version: *mut u16,
    out_value: *mut *const u8,
    out_value_length: *mut u64,
) {
    // SAFETY: The caller validated every output pointer as writable.
    unsafe {
        out_algorithm.write(fingerprint.algorithm.as_ptr());
        out_version.write(fingerprint.version);
        out_value.write(fingerprint.value.as_ptr());
        out_value_length.write(u64::try_from(fingerprint.value.len()).unwrap_or(u64::MAX));
    }
}

fn members(structure: &ContentStructure) -> Result<Vec<AbiMember>, Error> {
    if let Some(members) = structure.members() {
        return members
            .iter()
            .map(|member| {
                Ok(AbiMember {
                    resource_id: member.resource_id(),
                    role: Some(exact_cstring(member.role().as_str(), "resource role")?),
                    required: member.is_required(),
                })
            })
            .collect();
    }
    Ok(structure
        .resource_ids()
        .into_iter()
        .map(|resource_id| AbiMember {
            resource_id,
            role: None,
            required: true,
        })
        .collect())
}

fn sequence(structure: &ContentStructure) -> Option<AbiSequence> {
    let descriptor = structure.image_sequence_descriptor()?;
    let frames = descriptor.frames();
    let rate = descriptor.rate();
    Some(AbiSequence {
        start: frames.start(),
        end: frames.end(),
        step: frames.step(),
        rate_numerator: rate.numerator(),
        rate_denominator: rate.denominator(),
        missing_frames: descriptor.known_missing_frames().to_vec(),
    })
}

const fn representation_kind(kind: RepresentationKind) -> u32 {
    match kind {
        RepresentationKind::Original => PP_REPRESENTATION_ORIGINAL,
        RepresentationKind::Proxy => PP_REPRESENTATION_PROXY,
        RepresentationKind::Optimized => PP_REPRESENTATION_OPTIMIZED,
        RepresentationKind::Derived => PP_REPRESENTATION_DERIVED,
        _ => 0,
    }
}

const fn content_structure_kind(kind: ContentStructureKind) -> u32 {
    match kind {
        ContentStructureKind::SingleResource => PP_CONTENT_SINGLE_RESOURCE,
        ContentStructureKind::ImageSequence => PP_CONTENT_IMAGE_SEQUENCE,
        ContentStructureKind::OrderedParts => PP_CONTENT_ORDERED_PARTS,
        ContentStructureKind::Package => PP_CONTENT_PACKAGE,
        _ => 0,
    }
}

const fn locator_availability(availability: LocatorAvailability) -> u32 {
    match availability {
        LocatorAvailability::Unknown => PP_LOCATOR_UNKNOWN,
        LocatorAvailability::Online => PP_LOCATOR_ONLINE,
        LocatorAvailability::Offline => PP_LOCATOR_OFFLINE,
        _ => 0,
    }
}

fn invalid_argument(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidArgument, message)
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use postproject_core::{
        AssetId, ContentStructure, FrameRange, ImageSequenceDescriptor, RationalRate,
        Representation, RepresentationId, RepresentationKind, ResourceId,
    };

    use super::*;

    #[test]
    fn sequence_accessors_copy_compact_structure() {
        let resource_id = ResourceId::new();
        let descriptor = ImageSequenceDescriptor::new(
            resource_id,
            FrameRange::new(1001, 1005, 1).expect("valid range"),
            RationalRate::new(24_000, 1_001).expect("valid rate"),
            vec![1003],
        )
        .expect("valid sequence");
        let representation = Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Original,
            ContentStructure::image_sequence(descriptor),
            Vec::new(),
        );
        let set = PpRepresentationSet {
            representations: vec![
                AbiRepresentation::new(&representation, Vec::new()).expect("ABI representation"),
            ],
            next_cursor: None,
        };
        let mut start = 0;
        let mut end = 0;
        let mut step = 0;
        let mut rate_numerator = 0;
        let mut rate_denominator = 0;
        let mut missing_count = 0;
        let mut error = ptr::null_mut();

        // SAFETY: The set and every output remain live and writable for the call.
        let status = unsafe {
            pp_representation_set_get_sequence(
                &raw const set,
                0,
                &raw mut start,
                &raw mut end,
                &raw mut step,
                &raw mut rate_numerator,
                &raw mut rate_denominator,
                &raw mut missing_count,
                &raw mut error,
            )
        };
        assert_eq!(status, 0);
        assert!(error.is_null());
        assert_eq!((start, end, step), (1001, 1005, 1));
        assert_eq!((rate_numerator, rate_denominator), (24_000, 1_001));
        assert_eq!(missing_count, 1);

        let mut missing_frame = 0;
        // SAFETY: The set and output remain live and writable for the call.
        let status = unsafe {
            pp_representation_set_get_sequence_missing_frame(
                &raw const set,
                0,
                0,
                &raw mut missing_frame,
                &raw mut error,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(missing_frame, 1003);
    }
}
