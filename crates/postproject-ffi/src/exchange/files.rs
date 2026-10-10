//! Exclusive sealed file publication and bounded native receivers.

use std::{ffi::c_char, fs::File, path::Path};

use postproject_protocol::{FailureKind, Limits, ProtocolError};
use postproject_storage_sqlite::{
    CheckpointLimits, ExchangeResult, ReplayLimits,
    exchange_file::{self, FileLimits},
};

use super::{PpExchangeLimits, PpExchangePosition, error};
use crate::{
    PpError, PpProduction, initialize_output, initialize_value, invalid_argument, lock_production,
    require_output, required_utf8,
};

pub(super) fn budgets(
    limits: Option<&PpExchangeLimits>,
) -> ExchangeResult<(FileLimits, CheckpointLimits, ReplayLimits)> {
    let limits = limits.copied().unwrap_or_default();
    let checked_size = |value| {
        usize::try_from(value).map_err(|_| {
            ProtocolError::new(
                FailureKind::LimitExceeded,
                "receiver budget exceeds addressable size",
            )
        })
    };
    let document = Limits::new(
        checked_size(limits.document_bytes)?,
        usize::try_from(limits.document_depth).map_err(|_| {
            ProtocolError::new(FailureKind::LimitExceeded, "invalid document depth")
        })?,
        checked_size(limits.document_nodes)?,
    )?;
    Ok((
        FileLimits::new(limits.file_bytes, document)?,
        CheckpointLimits::new(
            limits.file_bytes,
            limits.disk_bytes,
            limits.frames,
            document,
        )?,
        ReplayLimits::new(limits.file_bytes, document)?,
    ))
}

pub(super) fn io(error: &std::io::Error, message: &'static str) -> crate::Error {
    let kind = if error.kind() == std::io::ErrorKind::AlreadyExists {
        crate::ErrorKind::AlreadyExists
    } else {
        crate::ErrorKind::Io
    };
    crate::Error::new(kind, message)
}

pub(super) fn publish<T>(
    path: &Path,
    write: impl FnOnce(&mut File) -> ExchangeResult<T>,
) -> ExchangeResult<T> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| io(&error, "create private exchange output"))?;
    let result = write(temporary.as_file_mut())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| io(&error, "flush complete exchange output"))?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| io(&error.error, "publish exclusive complete exchange output"))?;
    Ok(result)
}

/// Exports one pinned complete checkpoint to a new exclusively published file.
///
/// # Safety
/// Production live/serialized, path terminated UTF-8, position output writable,
/// error optional. Failure never replaces an existing public output file.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_export_checkpoint(
    production: *const PpProduction,
    path: *const c_char,
    out_position: *mut PpExchangePosition,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear output, then validate readable handle and path.
    unsafe {
        initialize_value(out_position, PpExchangePosition::empty());
        error::call(out_error, || {
            require_output(out_position, "out_position")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let path = Path::new(required_utf8(path, "checkpoint output path")?);
            let store = lock_production(&production.state);
            let manifest = publish(path, |file| exchange_file::write_checkpoint(&store, file))?;
            out_position.write(manifest.head().into());
            Ok(())
        })
    }
}

/// Validates every checkpoint section in private temporary storage.
///
/// # Safety
/// Path terminated UTF-8, optional limits readable, output writable, error
/// optional. Null limits selects SDK defaults. Returns the complete source head.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_checkpoint_validate_file(
    path: *const c_char,
    limits: *const PpExchangeLimits,
    out_position: *mut PpExchangePosition,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs initialized and optional limits copied before decoding.
    unsafe {
        initialize_value(out_position, PpExchangePosition::empty());
        error::call(out_error, || {
            require_output(out_position, "out_position")?;
            let (file_limits, checkpoint_limits, _) = budgets(limits.as_ref())?;
            let input = File::open(required_utf8(path, "checkpoint input path")?)
                .map_err(|error| io(&error, "open complete checkpoint input"))?;
            let directory = tempfile::tempdir()
                .map_err(|error| io(&error, "create private checkpoint validation directory"))?;
            let mirror = exchange_file::import_checkpoint(
                directory.path().join("validated.pproj"),
                input,
                file_limits,
                checkpoint_limits,
            )?;
            let position = mirror.exchange_head()?;
            drop(mirror);
            out_position.write(position.into());
            Ok(())
        })
    }
}

/// Creates a new passive production only after complete checkpoint validation.
///
/// # Safety
/// Both paths terminated UTF-8, optional limits readable, owning production
/// output writable, error optional. Existing destination is never overwritten.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_checkpoint_import_file(
    path: *const c_char,
    destination: *const c_char,
    limits: *const PpExchangeLimits,
    out_production: *mut *mut PpProduction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear owning output before parsing paths and receiver budgets.
    unsafe {
        initialize_output(out_production);
        error::call(out_error, || {
            require_output(out_production, "out_production")?;
            let (file_limits, checkpoint_limits, _) = budgets(limits.as_ref())?;
            let input = File::open(required_utf8(path, "checkpoint input path")?)
                .map_err(|error| io(&error, "open complete checkpoint input"))?;
            let destination = Path::new(required_utf8(destination, "mirror destination path")?);
            let mirror = exchange_file::import_checkpoint(
                destination,
                input,
                file_limits,
                checkpoint_limits,
            )?;
            out_production.write(Box::into_raw(Box::new(crate::production_handle(mirror))));
            Ok(())
        })
    }
}
