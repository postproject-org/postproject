//! Image-sequence file namings crossing the C boundary.

use std::{ffi::CString, os::raw::c_char, ptr};

use postproject_core::{Error, SequenceNaming};

use crate::{exact_cstring, required_utf8};

/// Borrowed naming of the files of an image sequence in one directory.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpSequenceNaming {
    /// Required NUL-terminated text before the frame number.
    pub prefix: *const c_char,
    /// Required NUL-terminated text after the frame number.
    pub suffix: *const c_char,
    /// Minimum number of frame-number digits.
    pub padding: u8,
}

/// A naming owned by a result set, lent to C callers as [`PpSequenceNaming`].
pub(crate) struct AbiSequenceNaming {
    prefix: CString,
    suffix: CString,
    padding: u8,
}

impl AbiSequenceNaming {
    pub(crate) fn new(naming: &SequenceNaming) -> Result<Self, Error> {
        Ok(Self {
            prefix: exact_cstring(naming.prefix(), "sequence naming prefix")?,
            suffix: exact_cstring(naming.suffix(), "sequence naming suffix")?,
            padding: naming.padding(),
        })
    }

    /// Copies a naming whose strings are known to contain no NUL.
    pub(crate) fn sanitized(naming: &SequenceNaming) -> Self {
        Self {
            prefix: crate::sanitized_cstring(naming.prefix()),
            suffix: crate::sanitized_cstring(naming.suffix()),
            padding: naming.padding(),
        }
    }

    pub(crate) fn optional(naming: Option<&SequenceNaming>) -> Result<Option<Self>, Error> {
        naming.map(Self::new).transpose()
    }

    fn borrowed(&self) -> PpSequenceNaming {
        PpSequenceNaming {
            prefix: self.prefix.as_ptr(),
            suffix: self.suffix.as_ptr(),
            padding: self.padding,
        }
    }
}

const EMPTY_NAMING: PpSequenceNaming = PpSequenceNaming {
    prefix: ptr::null(),
    suffix: ptr::null(),
    padding: 0,
};

/// Clears an optional naming output pair when both pointers are writable.
///
/// # Safety
///
/// Each pointer must be null or writable.
pub(crate) unsafe fn initialize_naming_output(
    out_has_naming: *mut u8,
    out_naming: *mut PpSequenceNaming,
) {
    // SAFETY: The caller guarantees null or writable pointers.
    unsafe {
        if !out_has_naming.is_null() {
            out_has_naming.write(0);
        }
        if !out_naming.is_null() {
            out_naming.write(EMPTY_NAMING);
        }
    }
}

/// Writes an optional naming to checked, writable outputs.
///
/// # Safety
///
/// Both pointers must be writable, and the naming must outlive the borrow the
/// caller documents.
pub(crate) unsafe fn write_naming_output(
    naming: Option<&AbiSequenceNaming>,
    out_has_naming: *mut u8,
    out_naming: *mut PpSequenceNaming,
) {
    // SAFETY: The caller guarantees writable outputs.
    unsafe {
        if let Some(naming) = naming {
            out_has_naming.write(1);
            out_naming.write(naming.borrowed());
        } else {
            out_has_naming.write(0);
            out_naming.write(EMPTY_NAMING);
        }
    }
}

/// Copies a nullable borrowed naming.
///
/// # Safety
///
/// `naming` must be null or point to a readable naming whose strings are
/// NUL-terminated UTF-8.
pub(crate) unsafe fn optional_naming(
    naming: *const PpSequenceNaming,
    label: &str,
) -> Result<Option<SequenceNaming>, Error> {
    // SAFETY: The caller guarantees a null or readable naming.
    unsafe {
        let Some(naming) = naming.as_ref() else {
            return Ok(None);
        };
        required_naming(naming, label).map(Some)
    }
}

/// Copies a borrowed naming.
///
/// # Safety
///
/// The strings must be null or NUL-terminated UTF-8.
pub(crate) unsafe fn required_naming(
    naming: &PpSequenceNaming,
    label: &str,
) -> Result<SequenceNaming, Error> {
    // SAFETY: The caller guarantees readable strings.
    unsafe {
        SequenceNaming::new(
            required_utf8(naming.prefix, &format!("{label}.prefix"))?,
            required_utf8(naming.suffix, &format!("{label}.suffix"))?,
            naming.padding,
        )
    }
}
