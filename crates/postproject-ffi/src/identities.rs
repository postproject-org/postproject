//! Explicit semantic identity projections; bytes retain their persisted meaning.

use std::{ffi::c_char, str::FromStr};

use postproject_core::{AssetId, LocatorId, MediaRootId, ProductionId, RevisionId, TransactionId};

use crate::{PpError, exact_cstring, ffi_call, initialize_output, require_output, required_utf8};

/// Production identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpProductionId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a production identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_id_parse(
    text: *const c_char,
    out_id: *mut PpProductionId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpProductionId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = ProductionId::from_str(required_utf8(text, "production ID")?)?;
            out_id.write(PpProductionId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a production identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_id_format(
    id: PpProductionId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(
                &ProductionId::from_bytes(id.bytes).to_string(),
                "production ID",
            )?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Revision identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpRevisionId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a revision identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_id_parse(
    text: *const c_char,
    out_id: *mut PpRevisionId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpRevisionId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = RevisionId::from_str(required_utf8(text, "revision ID")?)?;
            out_id.write(PpRevisionId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a revision identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_id_format(
    id: PpRevisionId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(&RevisionId::from_bytes(id.bytes).to_string(), "revision ID")?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Transaction identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpTransactionId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a transaction identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_id_parse(
    text: *const c_char,
    out_id: *mut PpTransactionId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpTransactionId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = TransactionId::from_str(required_utf8(text, "transaction ID")?)?;
            out_id.write(PpTransactionId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a transaction identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_id_format(
    id: PpTransactionId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(
                &TransactionId::from_bytes(id.bytes).to_string(),
                "transaction ID",
            )?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Asset identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpAssetId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a asset identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_asset_id_parse(
    text: *const c_char,
    out_id: *mut PpAssetId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpAssetId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = AssetId::from_str(required_utf8(text, "asset ID")?)?;
            out_id.write(PpAssetId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a asset identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_asset_id_format(
    id: PpAssetId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(&AssetId::from_bytes(id.bytes).to_string(), "asset ID")?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Media-root identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpMediaRootId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a media-root identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_media_root_id_parse(
    text: *const c_char,
    out_id: *mut PpMediaRootId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpMediaRootId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = MediaRootId::from_str(required_utf8(text, "media-root ID")?)?;
            out_id.write(PpMediaRootId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a media-root identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_media_root_id_format(
    id: PpMediaRootId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(
                &MediaRootId::from_bytes(id.bytes).to_string(),
                "media-root ID",
            )?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Locator identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpLocatorId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a locator identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_locator_id_parse(
    text: *const c_char,
    out_id: *mut PpLocatorId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpLocatorId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = LocatorId::from_str(required_utf8(text, "locator ID")?)?;
            out_id.write(PpLocatorId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a locator identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_locator_id_format(
    id: PpLocatorId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(&LocatorId::from_bytes(id.bytes).to_string(), "locator ID")?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Constructs an asset reference; existence and scope remain operation checks.
///
/// # Safety
/// Output must be writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_from_asset(
    id: PpAssetId,
    out_ref: *mut crate::PpObjectRef,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and checked before writing.
    unsafe {
        crate::initialize_value(
            out_ref,
            crate::PpObjectRef {
                kind: 0,
                id: crate::PpUuid { bytes: [0; 16] },
            },
        );
        ffi_call(out_error, || {
            require_output(out_ref, "out_ref")?;
            out_ref.write(crate::PpObjectRef {
                kind: crate::PP_OBJECT_ASSET,
                id: crate::PpUuid { bytes: id.bytes },
            });
            Ok(())
        })
    }
}

/// Reads an asset identity from a matching object-reference kind.
///
/// # Safety
/// Reference must be readable, output writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_get_asset(
    value: *const crate::PpObjectRef,
    out_id: *mut PpAssetId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and pointers checked before use.
    unsafe {
        crate::initialize_value(out_id, PpAssetId { bytes: [0; 16] });
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let value = value
                .as_ref()
                .ok_or_else(|| crate::invalid_argument("reference must not be null"))?;
            if value.kind != crate::PP_OBJECT_ASSET {
                return Err(crate::invalid_argument("reference must name an asset"));
            }
            out_id.write(PpAssetId {
                bytes: value.id.bytes,
            });
            Ok(())
        })
    }
}
