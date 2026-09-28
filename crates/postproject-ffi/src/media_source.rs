//! Caller-owned media sources for import and representation creation.

use std::{
    os::raw::c_char,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};

use postproject_core::{
    ContentStructure, Error, FrameRange, ImageSequenceDescriptor, ImageSequencePattern,
    MAX_CONTENT_MEMBERS, MAX_SEQUENCE_EXCEPTIONS, RationalRate, ResourceId, ResourceMember,
    ResourceRole,
};
use postproject_media::{FileResourceSource, ImageSequenceSource, MediaSource};

use crate::{
    PpError, PpFileResourceInput, ffi_call, initialize_output, invalid_argument, require_output,
    required_utf8,
};

/// Opaque, caller-owned description of a representation's content structure
/// at its present location.
pub struct PpMediaSource {
    pub(crate) source: MediaSource,
}

/// Creates a single-file media source.
///
/// # Safety
///
/// `path` must be borrowed NUL-terminated UTF-8. `out_source` must be
/// writable; `out_error` may be null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_media_source_create_file(
    path: *const c_char,
    out_source: *mut *mut PpMediaSource,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The output is validated before it is written, and the path is
    // copied before this call returns.
    unsafe {
        initialize_output(out_source);
        ffi_call(out_error, || {
            require_output(out_source, "out_source")?;
            let path = required_path(path, "path")?;
            write_source(out_source, MediaSource::File(Path::new(path).to_path_buf()));
            Ok(())
        })
    }
}

/// Creates a compact image-sequence media source.
///
/// # Safety
///
/// String pointers must be borrowed NUL-terminated UTF-8. `missing_frames`
/// must point to `missing_frame_count` readable values and may be null only
/// when the count is zero. `out_source` must be writable; `out_error` may be
/// null or writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn pp_media_source_create_image_sequence(
    directory: *const c_char,
    prefix: *const c_char,
    suffix: *const c_char,
    padding: u8,
    start: i64,
    end: i64,
    step: u32,
    rate_numerator: u32,
    rate_denominator: u32,
    missing_frames: *const i64,
    missing_frame_count: u64,
    out_source: *mut *mut PpMediaSource,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers and counts are validated before borrowed values are
    // read, and every value is copied before this call returns.
    unsafe {
        initialize_output(out_source);
        ffi_call(out_error, || {
            require_output(out_source, "out_source")?;
            let directory = required_path(directory, "directory")?;
            let missing_frame_count = usize::try_from(missing_frame_count)
                .map_err(|_| invalid_argument("missing frame count is too large"))?;
            if missing_frame_count > MAX_SEQUENCE_EXCEPTIONS {
                return Err(invalid_argument(format!(
                    "missing frame count must not exceed {MAX_SEQUENCE_EXCEPTIONS}"
                )));
            }
            let missing_frames = if missing_frame_count == 0 {
                Vec::new()
            } else {
                if missing_frames.is_null() {
                    return Err(invalid_argument(
                        "missing_frames must not be null when count is nonzero",
                    ));
                }
                // SAFETY: The caller guarantees the checked count of readable values.
                std::slice::from_raw_parts(missing_frames, missing_frame_count).to_vec()
            };
            let pattern = ImageSequencePattern::new(
                required_utf8(prefix, "prefix")?,
                required_utf8(suffix, "suffix")?,
                padding,
            )?;
            let frames = FrameRange::new(start, end, step)?;
            let rate = RationalRate::new(rate_numerator, rate_denominator)?;
            // The descriptor rules are checked now so an invalid source never
            // reaches a transaction.
            ImageSequenceDescriptor::new(
                ResourceId::new(),
                pattern.clone(),
                frames,
                rate,
                missing_frames.clone(),
            )?;
            let source = ImageSequenceSource::new(
                Path::new(directory),
                pattern,
                frames,
                rate,
                missing_frames,
            );
            write_source(out_source, MediaSource::ImageSequence(source));
            Ok(())
        })
    }
}

/// Creates an ordered, fully required multi-file media source.
///
/// # Safety
///
/// `members` must point to `member_count` readable values whose strings are
/// borrowed NUL-terminated UTF-8; it may be null only when the count is zero.
/// `out_source` must be writable; `out_error` may be null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_media_source_create_ordered_parts(
    members: *const PpFileResourceInput,
    member_count: u64,
    out_source: *mut *mut PpMediaSource,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: This exported function forwards the same pointer contract.
    unsafe { create_file_collection(members, member_count, out_source, out_error, false) }
}

/// Creates a role-bearing package media source.
///
/// # Safety
///
/// Pointer rules match [`pp_media_source_create_ordered_parts`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_media_source_create_package(
    members: *const PpFileResourceInput,
    member_count: u64,
    out_source: *mut *mut PpMediaSource,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: This exported function forwards the same pointer contract.
    unsafe { create_file_collection(members, member_count, out_source, out_error, true) }
}

/// Releases a media source. Null is a no-op.
///
/// # Safety
///
/// `source` must be null or a live handle not used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_media_source_release(source: *mut PpMediaSource) {
    if source.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership of a live allocation is transferred exactly once.
        drop(unsafe { Box::from_raw(source) });
    }));
}

/// Borrows the source behind a caller-supplied handle.
///
/// # Safety
///
/// `source` must be null or live for the returned lifetime.
pub(crate) unsafe fn borrowed_source<'a>(
    source: *const PpMediaSource,
) -> Result<&'a MediaSource, Error> {
    // SAFETY: A non-null handle is live by the caller contract.
    unsafe { source.as_ref() }
        .map(|source| &source.source)
        .ok_or_else(|| invalid_argument("source must not be null"))
}

unsafe fn create_file_collection(
    members: *const PpFileResourceInput,
    member_count: u64,
    out_source: *mut *mut PpMediaSource,
    out_error: *mut *mut PpError,
    package: bool,
) -> u32 {
    // SAFETY: Null pointers and counts are validated before borrowed values are
    // read, and every value is copied before this call returns.
    unsafe {
        initialize_output(out_source);
        ffi_call(out_error, || {
            require_output(out_source, "out_source")?;
            let member_count = usize::try_from(member_count)
                .map_err(|_| invalid_argument("member count is too large"))?;
            if member_count > MAX_CONTENT_MEMBERS {
                return Err(invalid_argument(format!(
                    "member count must not exceed {MAX_CONTENT_MEMBERS}"
                )));
            }
            let members = if member_count == 0 {
                &[]
            } else {
                if members.is_null() {
                    return Err(invalid_argument(
                        "members must not be null when count is nonzero",
                    ));
                }
                // SAFETY: The caller guarantees the checked count of readable values.
                std::slice::from_raw_parts(members, member_count)
            };
            let mut sources = Vec::with_capacity(members.len());
            let mut structure_members = Vec::with_capacity(members.len());
            for member in members {
                let required = match member.required {
                    0 => false,
                    1 => true,
                    _ => {
                        return Err(invalid_argument("member required flag must be zero or one"));
                    }
                };
                let path = required_path(member.path, "member path")?;
                let role = ResourceRole::new(required_utf8(member.role, "member role")?)?;
                structure_members.push(ResourceMember::new(
                    ResourceId::new(),
                    role.clone(),
                    required,
                ));
                sources.push(FileResourceSource::new(Path::new(path), role, required));
            }
            // Structural rules are checked now so an invalid source never
            // reaches a transaction.
            let source = if package {
                ContentStructure::package(structure_members)?;
                MediaSource::Package(sources)
            } else {
                ContentStructure::ordered_parts(structure_members)?;
                MediaSource::OrderedParts(sources)
            };
            write_source(out_source, source);
            Ok(())
        })
    }
}

unsafe fn required_path<'a>(value: *const c_char, label: &str) -> Result<&'a str, Error> {
    // SAFETY: The caller forwards a borrowed NUL-terminated string contract.
    let value = unsafe { required_utf8(value, label) }?;
    if value.is_empty() {
        return Err(invalid_argument(format!("{label} must not be empty")));
    }
    Ok(value)
}

unsafe fn write_source(out_source: *mut *mut PpMediaSource, source: MediaSource) {
    // SAFETY: The caller validated that the output is writable.
    unsafe { out_source.write(Box::into_raw(Box::new(PpMediaSource { source }))) };
}
