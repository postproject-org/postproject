//! Snapshot point reads and bounded pages using existing owned projections.

use crate::{
    AbiAsset, PpAssetSet, PpError, PpReadSession, PpRepresentationSet, PpUuid, ffi_call,
    initialize_output, invalid_argument, lock_production, query_cursor_to_cstring,
    query_page_request, require_output,
};
use postproject_core::{AssetId, Error, RepresentationId};
use std::ffi::c_char;

// Reuse live-read validation and owned projections on the pinned connection.
// NULL sessions delegate as NULL productions, which clear every output.
unsafe fn forward_read(
    session: *const PpReadSession,
    out_error: *mut *mut PpError,
    operation: impl FnOnce(*const crate::PpProduction) -> u32,
) -> u32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: A non-null session is live by the exported caller contract.
        let reader = unsafe { session.as_ref() }
            .map_or(std::ptr::null(), |session| &raw const session.reader);
        operation(reader)
    }))
    .unwrap_or_else(|_| {
        // SAFETY: Error output validity is the exported caller contract.
        unsafe {
            ffi_call(out_error, || {
                Err(Error::new(
                    postproject_core::ErrorKind::Internal,
                    "read projection panicked",
                ))
            })
        }
    })
}

/// Copies a bounded resource page from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_resources_page(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_resources_page(
                reader,
                representation_id,
                limit,
                cursor,
                out_objects,
                out_error,
            )
        })
    }
}

/// Copies a bounded locator page from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_locators_page(
    session: *const PpReadSession,
    resource_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_locators: *mut *mut crate::PpLocatorQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_locators_page(
                reader,
                resource_id,
                limit,
                cursor,
                out_locators,
                out_error,
            )
        })
    }
}

/// Copies representations using a resource from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_representations_using_resource(
    session: *const PpReadSession,
    resource_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::representations::pp_production_representations_using_resource(
                reader,
                resource_id,
                limit,
                cursor,
                out_representations,
                out_error,
            )
        })
    }
}

/// Copies logical roots from the pinned view into an owned set.
///
/// # Safety
/// Session must be live; outputs writable, with a nullable error output.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_media_roots(
    session: *const PpReadSession,
    out_roots: *mut *mut crate::PpMediaRootSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_media_roots(reader, out_roots, out_error)
        })
    }
}

/// Resolves current files using storage knowledge from the pinned view.
///
/// # Safety
/// Session must be live; IDs readable for count (null for zero), options null
/// or live, outputs writable. The filesystem is not part of the database view.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_resolve_assets(
    session: *const PpReadSession,
    asset_ids: *const PpUuid,
    asset_count: u64,
    options: *const crate::PpResolutionOptions,
    out_resolutions: *mut *mut crate::PpResolutionSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::resolution::pp_production_resolve_assets(
                reader,
                asset_ids,
                asset_count,
                options,
                out_resolutions,
                out_error,
            )
        })
    }
}

/// Compares current files with resource fingerprints from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; path UTF-8/NUL-terminated, naming null or
/// readable, outputs writable. Filesystem contents may change independently.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_verify_resource(
    session: *const PpReadSession,
    resource_id: *const PpUuid,
    path: *const c_char,
    sequence_naming: *const crate::PpSequenceNaming,
    out_verification: *mut u32,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::content::pp_production_verify_resource(
                reader,
                resource_id,
                path,
                sequence_naming,
                out_verification,
                out_error,
            )
        })
    }
}

/// Copies an object's metadata from the pinned view.
///
/// # Safety
/// Session/target must be live/readable; outputs writable or error output null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_metadata(
    session: *const PpReadSession,
    target: *const crate::PpObjectRef,
    out_metadata: *mut *mut crate::PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_metadata(reader, target, out_metadata, out_error)
        })
    }
}

/// Copies assertions of one property from the pinned view.
///
/// # Safety
/// Session must be live; strings UTF-8/NUL-terminated, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_find_metadata(
    session: *const PpReadSession,
    vocabulary: *const c_char,
    property: *const c_char,
    out_metadata: *mut *mut crate::PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_find_metadata(
                reader,
                vocabulary,
                property,
                out_metadata,
                out_error,
            )
        })
    }
}

/// Copies a bounded metadata page from the pinned view.
///
/// # Safety
/// Session must be live; strings UTF-8/NUL-terminated, exact value readable or
/// null, cursor nullable UTF-8/NUL-terminated, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_query_metadata(
    session: *const PpReadSession,
    vocabulary: *const c_char,
    property: *const c_char,
    exact_value: *const crate::PpMetadataInput,
    limit: u32,
    cursor: *const c_char,
    out_metadata: *mut *mut crate::PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_query_metadata(
                reader,
                vocabulary,
                property,
                exact_value,
                limit,
                cursor,
                out_metadata,
                out_error,
            )
        })
    }
}

/// Copies external identifiers from the pinned view into an owned set.
///
/// # Safety
/// Session/target must be live/readable; outputs writable or error output null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_external_identifiers(
    session: *const PpReadSession,
    target: *const crate::PpObjectRef,
    out_identifiers: *mut *mut crate::PpExternalIdentifierSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_external_identifiers(reader, target, out_identifiers, out_error)
        })
    }
}

/// Looks up exact external identifiers in the pinned view.
///
/// # Safety
/// Session must be live; strings UTF-8/NUL-terminated (qualifier nullable), outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_find_by_external_identifier(
    session: *const PpReadSession,
    scheme: *const c_char,
    value: *const c_char,
    qualifier: *const c_char,
    out_objects: *mut *mut crate::PpObjectRefSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_find_by_external_identifier(
                reader,
                scheme,
                value,
                qualifier,
                out_objects,
                out_error,
            )
        })
    }
}

/// Finds known media by exact locator identity in the pinned view.
///
/// # Safety
/// Session must be live; URI UTF-8/NUL-terminated, optional naming readable,
/// cursor nullable UTF-8/NUL-terminated, outputs writable or error output null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_find_known_media_by_locator(
    session: *const PpReadSession,
    uri: *const c_char,
    sequence_naming: *const crate::PpSequenceNaming,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut crate::PpKnownMediaSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::known_media::pp_production_find_known_media_by_locator(
                reader,
                uri,
                sequence_naming,
                limit,
                cursor,
                out_matches,
                out_error,
            )
        })
    }
}

/// Finds known media by fingerprint evidence in the pinned view.
///
/// # Safety
/// Session must be live; algorithm UTF-8/NUL-terminated, value readable for length,
/// cursor nullable UTF-8/NUL-terminated, outputs writable or error output null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_find_known_media_by_fingerprint(
    session: *const PpReadSession,
    algorithm: *const c_char,
    version: u16,
    value: *const u8,
    value_length: u64,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut crate::PpKnownMediaSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::known_media::pp_production_find_known_media_by_fingerprint(
                reader,
                algorithm,
                version,
                value,
                value_length,
                limit,
                cursor,
                out_matches,
                out_error,
            )
        })
    }
}

/// Reads copied values from the session's pinned view.
///
/// # Safety
///
/// Session and IDs must be live/readable, cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_assets_page(
    session: *const PpReadSession,
    limit: u32,
    cursor: *const c_char,
    out_assets: *mut *mut PpAssetSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_assets);
        ffi_call(out_error, || {
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            require_output(out_assets, "out_assets")?;
            let page_request = query_page_request(limit, cursor)?;
            let page = lock_production(&session.reader.state).assets_page(&page_request)?;
            let assets = page
                .items()
                .iter()
                .map(AbiAsset::try_from)
                .collect::<Result<Vec<_>, Error>>()?;
            out_assets.write(Box::into_raw(Box::new(PpAssetSet {
                assets,
                next_cursor: query_cursor_to_cstring(page.next_cursor())?,
            })));
            Ok(())
        })
    }
}

/// Reads copied values from the session's pinned view.
///
/// # Safety
///
/// Session and IDs must be live/readable, cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_asset(
    session: *const PpReadSession,
    asset_id: *const PpUuid,
    out_assets: *mut *mut PpAssetSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and pointers checked before use.
    unsafe {
        initialize_output(out_assets);
        ffi_call(out_error, || {
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            let asset_id = asset_id
                .as_ref()
                .ok_or_else(|| invalid_argument("asset_id must not be null"))?;
            require_output(out_assets, "out_assets")?;
            let asset = lock_production(&session.reader.state)
                .asset(AssetId::from_bytes(asset_id.bytes))?;
            out_assets.write(Box::into_raw(Box::new(PpAssetSet {
                assets: vec![AbiAsset::try_from(&asset)?],
                next_cursor: None,
            })));
            Ok(())
        })
    }
}

/// Reads copied values from the session's pinned view.
///
/// # Safety
///
/// Session and IDs must be live/readable, cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_representations_page(
    session: *const PpReadSession,
    asset_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            let asset_id = asset_id
                .as_ref()
                .ok_or_else(|| invalid_argument("asset_id must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let page_request = query_page_request(limit, cursor)?;
            let inner = lock_production(&session.reader.state);
            let page =
                inner.representations_page(AssetId::from_bytes(asset_id.bytes), &page_request)?;
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet::new_page(
                &*inner,
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

/// Reads an owned representation from the pinned view.
///
/// # Safety
///
/// Session and ID must be live/readable and output pointers valid.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_representation(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            let representation_id = representation_id
                .as_ref()
                .ok_or_else(|| invalid_argument("representation_id must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let inner = lock_production(&session.reader.state);
            let representation =
                inner.representation(RepresentationId::from_bytes(representation_id.bytes))?;
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet::new_page(
                &*inner,
                &[representation],
                None,
            )?)));
            Ok(())
        })
    }
}
