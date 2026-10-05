//! Bounded known-media lookup across the C boundary.

use std::{ffi::CString, os::raw::c_char, ptr};

use postproject_core::{
    AssetId, KnownMediaMatch, LocatorIdentity, RepresentationId, ResourceFingerprint, ResourceId,
};

use crate::{
    PpAssetId, PpError, PpProduction, PpSequenceNaming, PpUuid, ffi_call, initialize_output,
    initialize_uuid, initialize_value, invalid_argument, item_at, lock_production, optional_naming,
    query_cursor_to_cstring, query_page_request, require_output, required_bytes, required_utf8,
};

/// Opaque immutable known-media result set owned by the C caller.
pub struct PpKnownMediaSet {
    matches: Vec<AbiKnownMediaMatch>,
    next_cursor: Option<CString>,
}

#[derive(Clone, Copy)]
struct AbiKnownMediaMatch {
    asset: AssetId,
    representation: RepresentationId,
    resource: ResourceId,
}

impl From<&KnownMediaMatch> for AbiKnownMediaMatch {
    fn from(value: &KnownMediaMatch) -> Self {
        Self {
            asset: value.asset().id(),
            representation: value.representation().id(),
            resource: value.resource().id(),
        }
    }
}

/// Finds current resources by exact canonical locator identity.
///
/// # Safety
///
/// `production` must be live, `uri` must be borrowed NUL-terminated UTF-8,
/// `sequence_naming` must be null or readable, `cursor` must be null or
/// borrowed NUL-terminated UTF-8, `out_matches` must be writable, and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_find_known_media_by_locator(
    production: *const PpProduction,
    uri: *const c_char,
    sequence_naming: *const PpSequenceNaming,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut PpKnownMediaSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are copied or validated before use; output is initialized.
    unsafe {
        initialize_output(out_matches);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_matches, "out_matches")?;
            let locator = LocatorIdentity::new(
                required_utf8(uri, "uri")?,
                optional_naming(sequence_naming, "sequence_naming")?,
            )?;
            let request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let page = inner.find_known_media_by_locator(&locator, &request)?;
            out_matches.write(Box::into_raw(Box::new(PpKnownMediaSet {
                matches: page.items().iter().map(AbiKnownMediaMatch::from).collect(),
                next_cursor: query_cursor_to_cstring(page.next_cursor())?,
            })));
            Ok(())
        })
    }
}

/// Finds current resources by exact resource-fingerprint evidence.
///
/// # Safety
///
/// `production` must be live, `algorithm` must be borrowed NUL-terminated
/// UTF-8, `value` must be readable for `value_length` bytes, `cursor` must be
/// null or borrowed NUL-terminated UTF-8, `out_matches` must be writable, and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_find_known_media_by_fingerprint(
    production: *const PpProduction,
    algorithm: *const c_char,
    version: u16,
    value: *const u8,
    value_length: u64,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut PpKnownMediaSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are copied or validated before use; output is initialized.
    unsafe {
        initialize_output(out_matches);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_matches, "out_matches")?;
            let fingerprint = ResourceFingerprint::new(
                required_utf8(algorithm, "algorithm")?,
                version,
                required_bytes(value, value_length, "value")?.to_vec(),
            )?;
            let request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let page = inner.find_known_media_by_fingerprint(&fingerprint, &request)?;
            out_matches.write(Box::into_raw(Box::new(PpKnownMediaSet {
                matches: page.items().iter().map(AbiKnownMediaMatch::from).collect(),
                next_cursor: query_cursor_to_cstring(page.next_cursor())?,
            })));
            Ok(())
        })
    }
}

/// Returns the number of known-media matches. Null input returns zero.
///
/// # Safety
///
/// `matches` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_known_media_set_count(matches: *const PpKnownMediaSet) -> u64 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { matches.as_ref() }.map_or(0, |set| {
            u64::try_from(set.matches.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Returns the borrowed next-page cursor, or null for the last page.
///
/// # Safety
///
/// `matches` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_known_media_set_next_cursor(
    matches: *const PpKnownMediaSet,
) -> *const c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { matches.as_ref() }
            .and_then(|set| set.next_cursor.as_ref())
            .map_or(ptr::null(), |cursor| cursor.as_ptr())
    }))
    .unwrap_or(ptr::null())
}

/// Reads the asset, representation, and resource identities of one match.
///
/// # Safety
///
/// `matches` must be live and every output must be writable. `out_error` may
/// be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_known_media_set_get(
    matches: *const PpKnownMediaSet,
    index: u64,
    out_asset_id: *mut PpAssetId,
    out_representation_id: *mut PpUuid,
    out_resource_id: *mut PpUuid,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_asset_id, PpAssetId { bytes: [0; 16] });
        initialize_uuid(out_representation_id);
        initialize_uuid(out_resource_id);
        ffi_call(out_error, || {
            require_output(out_asset_id, "out_asset_id")?;
            require_output(out_representation_id, "out_representation_id")?;
            require_output(out_resource_id, "out_resource_id")?;
            let set = matches
                .as_ref()
                .ok_or_else(|| invalid_argument("matches must not be null"))?;
            let value = item_at(&set.matches, index, "known-media match")?;
            out_asset_id.write(PpAssetId {
                bytes: value.asset.into_bytes(),
            });
            out_representation_id.write(PpUuid {
                bytes: value.representation.into_bytes(),
            });
            out_resource_id.write(PpUuid {
                bytes: value.resource.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Releases a known-media result set. Null is a no-op.
///
/// # Safety
///
/// A non-null pointer must be an unreleased result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_known_media_set_release(matches: *mut PpKnownMediaSet) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !matches.is_null() {
            // SAFETY: Ownership is transferred back exactly once by contract.
            drop(unsafe { Box::from_raw(matches) });
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_and_cursor_are_null_safe() {
        // SAFETY: Null is explicitly supported by both operations.
        unsafe {
            assert_eq!(pp_known_media_set_count(ptr::null()), 0);
            assert!(pp_known_media_set_next_cursor(ptr::null()).is_null());
        }
    }
}
