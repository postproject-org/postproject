//! Snapshot point reads and bounded pages using existing owned projections.

use crate::{
    AbiAsset, PpAssetSet, PpError, PpReadSession, PpRepresentationSet, PpUuid, ffi_call,
    initialize_output, invalid_argument, lock_production, query_cursor_to_cstring,
    query_page_request, require_output,
};
use postproject_core::{AssetId, Error, RepresentationId};
use std::ffi::c_char;

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
