//! Explicit semantic identity projections; bytes retain their persisted meaning.

use std::{ffi::c_char, str::FromStr};

use postproject_core::ProductionId;

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
