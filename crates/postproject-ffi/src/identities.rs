//! Explicit semantic identity projections; bytes retain their persisted meaning.

use std::{ffi::c_char, str::FromStr};

use postproject_core::{
    ActivityId, AssetId, JobId, LocatorId, MediaRootId, ProductionId, RepresentationId, ResourceId,
    RevisionId, TransactionId,
};

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

/// Parses UUID text as an asset identity without checking existence.
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

/// Formats an asset identity as owned canonical lowercase UUID text.
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

/// Job identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpJobId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a job identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_id_parse(
    text: *const c_char,
    out_id: *mut PpJobId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpJobId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = JobId::from_str(required_utf8(text, "job ID")?)?;
            out_id.write(PpJobId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a job identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_id_format(
    id: PpJobId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(&JobId::from_bytes(id.bytes).to_string(), "job ID")?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Activity identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpActivityId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as an activity identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_id_parse(
    text: *const c_char,
    out_id: *mut PpActivityId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpActivityId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = ActivityId::from_str(required_utf8(text, "activity ID")?)?;
            out_id.write(PpActivityId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats an activity identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_id_format(
    id: PpActivityId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(&ActivityId::from_bytes(id.bytes).to_string(), "activity ID")?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Representation identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpRepresentationId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a representation identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_id_parse(
    text: *const c_char,
    out_id: *mut PpRepresentationId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpRepresentationId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = RepresentationId::from_str(required_utf8(text, "representation ID")?)?;
            out_id.write(PpRepresentationId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a representation identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_representation_id_format(
    id: PpRepresentationId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(
                &RepresentationId::from_bytes(id.bytes).to_string(),
                "representation ID",
            )?;
            out_text.write(text.into_raw());
            Ok(())
        })
    }
}

/// Resource identity, distinct from interchangeable UUID bytes in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpResourceId {
    /// Stable UUID bytes; existence and membership are checked by the store.
    pub bytes: [u8; 16],
}

/// Parses UUID text as a resource identity without checking existence.
///
/// # Safety
/// Text must be UTF-8/NUL-terminated; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resource_id_parse(
    text: *const c_char,
    out_id: *mut PpResourceId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Writable output is initialized; strings/pointers validated.
    unsafe {
        if !out_id.is_null() {
            out_id.write(PpResourceId { bytes: [0; 16] });
        }
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let id = ResourceId::from_str(required_utf8(text, "resource ID")?)?;
            out_id.write(PpResourceId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

/// Formats a resource identity as owned canonical lowercase UUID text.
///
/// # Safety
/// Output must be writable, error nullable/writable. Release text with
/// `pp_string_release` after success.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resource_id_format(
    id: PpResourceId,
    out_text: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and validated before writing.
    unsafe {
        initialize_output(out_text);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            let text = exact_cstring(&ResourceId::from_bytes(id.bytes).to_string(), "resource ID")?;
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

/// Constructs a job reference; existence and scope remain operation checks.
///
/// # Safety
/// Output must be writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_from_job(
    id: PpJobId,
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
                kind: crate::PP_OBJECT_JOB,
                id: crate::PpUuid { bytes: id.bytes },
            });
            Ok(())
        })
    }
}

/// Reads a job identity from a matching object-reference kind.
///
/// # Safety
/// Reference must be readable, output writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_get_job(
    value: *const crate::PpObjectRef,
    out_id: *mut PpJobId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and pointers checked before use.
    unsafe {
        crate::initialize_value(out_id, PpJobId { bytes: [0; 16] });
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let value = value
                .as_ref()
                .ok_or_else(|| crate::invalid_argument("reference must not be null"))?;
            if value.kind != crate::PP_OBJECT_JOB {
                return Err(crate::invalid_argument("reference must name an job"));
            }
            out_id.write(PpJobId {
                bytes: value.id.bytes,
            });
            Ok(())
        })
    }
}

/// Constructs an activity reference; existence and scope remain operation checks.
///
/// # Safety
/// Output must be writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_from_activity(
    id: PpActivityId,
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
                kind: crate::PP_OBJECT_ACTIVITY,
                id: crate::PpUuid { bytes: id.bytes },
            });
            Ok(())
        })
    }
}

/// Reads an activity identity from a matching object-reference kind.
///
/// # Safety
/// Reference must be readable, output writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_get_activity(
    value: *const crate::PpObjectRef,
    out_id: *mut PpActivityId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and pointers checked before use.
    unsafe {
        crate::initialize_value(out_id, PpActivityId { bytes: [0; 16] });
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let value = value
                .as_ref()
                .ok_or_else(|| crate::invalid_argument("reference must not be null"))?;
            if value.kind != crate::PP_OBJECT_ACTIVITY {
                return Err(crate::invalid_argument("reference must name an activity"));
            }
            out_id.write(PpActivityId {
                bytes: value.id.bytes,
            });
            Ok(())
        })
    }
}

/// Constructs a representation reference; existence and scope remain operation checks.
///
/// # Safety
/// Output must be writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_from_representation(
    id: PpRepresentationId,
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
                kind: crate::PP_OBJECT_REPRESENTATION,
                id: crate::PpUuid { bytes: id.bytes },
            });
            Ok(())
        })
    }
}

/// Reads a representation identity from a matching object-reference kind.
///
/// # Safety
/// Reference must be readable, output writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_get_representation(
    value: *const crate::PpObjectRef,
    out_id: *mut PpRepresentationId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and pointers checked before use.
    unsafe {
        crate::initialize_value(out_id, PpRepresentationId { bytes: [0; 16] });
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let value = value
                .as_ref()
                .ok_or_else(|| crate::invalid_argument("reference must not be null"))?;
            if value.kind != crate::PP_OBJECT_REPRESENTATION {
                return Err(crate::invalid_argument(
                    "reference must name a representation",
                ));
            }
            out_id.write(PpRepresentationId {
                bytes: value.id.bytes,
            });
            Ok(())
        })
    }
}

/// Constructs a resource reference; existence and scope remain operation checks.
///
/// # Safety
/// Output must be writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_from_resource(
    id: PpResourceId,
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
                kind: crate::PP_OBJECT_RESOURCE,
                id: crate::PpUuid { bytes: id.bytes },
            });
            Ok(())
        })
    }
}

/// Reads a resource identity from a matching object-reference kind.
///
/// # Safety
/// Reference must be readable, output writable; error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_get_resource(
    value: *const crate::PpObjectRef,
    out_id: *mut PpResourceId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and pointers checked before use.
    unsafe {
        crate::initialize_value(out_id, PpResourceId { bytes: [0; 16] });
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            let value = value
                .as_ref()
                .ok_or_else(|| crate::invalid_argument("reference must not be null"))?;
            if value.kind != crate::PP_OBJECT_RESOURCE {
                return Err(crate::invalid_argument("reference must name a resource"));
            }
            out_id.write(PpResourceId {
                bytes: value.id.bytes,
            });
            Ok(())
        })
    }
}
