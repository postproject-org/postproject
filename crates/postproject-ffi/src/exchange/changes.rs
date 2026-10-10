//! Pinned range export and restart-safe passive application.

use std::{
    ffi::c_char,
    fs::File,
    path::Path,
    sync::{MutexGuard, atomic::Ordering},
};

use postproject_protocol::{FailureKind, Position, ProtocolBase, ProtocolError, StoreRole};
use postproject_storage_sqlite::{ResynchronizationLimits, exchange_file};

use super::{PpExchangeLimits, PpExchangePosition, PpProtocolBase, error, files};
use crate::{
    PpError, PpProduction, SqliteProduction, initialize_value, invalid_argument, lock_production,
    require_output, required_utf8,
};

fn idle_store(production: &PpProduction) -> Result<MutexGuard<'_, SqliteProduction>, crate::Error> {
    let store = lock_production(&production.state);
    if production.state.transaction_open.load(Ordering::Acquire) {
        return Err(crate::Error::new(
            crate::ErrorKind::Conflict,
            "production already has an open transaction",
        ));
    }
    Ok(store)
}

/// Exports complete records after an exact source position through a pinned head.
///
/// # Safety
/// Production live/serialized, from readable, path terminated UTF-8, output
/// writable, error optional. Output exclusively published after a complete seal.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_export_changes(
    production: *const PpProduction,
    from: *const PpExchangePosition,
    path: *const c_char,
    out_position: *mut PpExchangePosition,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs cleared and source position copied before export.
    unsafe {
        initialize_value(out_position, PpExchangePosition::empty());
        error::call(out_error, || {
            require_output(out_position, "out_position")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let from = from
                .as_ref()
                .ok_or_else(|| invalid_argument("source position must not be null"))?;
            let from = Position::try_from(*from)?;
            let path = Path::new(required_utf8(path, "changes output path")?);
            let store = lock_production(&production.state);
            let through = files::publish(path, |file| {
                exchange_file::write_changes(&store, from, file)
            })?;
            out_position.write(through.into());
            Ok(())
        })
    }
}

/// Applies complete original records to a passive mirror, retaining their prefix.
///
/// Failure can leave earlier complete records durable. Reopen, inspect and retry
/// the exact same file; verified duplicates create no new revision.
///
/// # Safety
/// Production live/serialized, path terminated UTF-8, optional limits readable,
/// output writable, error optional. An open native edit rejects before applying.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_apply_changes(
    production: *const PpProduction,
    path: *const c_char,
    limits: *const PpExchangeLimits,
    out_position: *mut PpExchangePosition,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear copied output before checked path and budget construction.
    unsafe {
        initialize_value(out_position, PpExchangePosition::empty());
        error::call(out_error, || {
            require_output(out_position, "out_position")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let (file_limits, _, replay_limits) = files::budgets(limits.as_ref())?;
            let input = File::open(required_utf8(path, "changes input path")?)
                .map_err(|error| files::io(&error, "open complete changes input"))?;
            let mut store = idle_store(production)?;
            let through =
                exchange_file::apply_changes(&mut store, input, file_limits, replay_limits)?;
            out_position.write(through.into());
            Ok(())
        })
    }
}

/// Explicitly establishes a new floor only for incomplete development history.
///
/// Requires a fresh original-history decision. Preserves earlier evidence and
/// outcomes without a domain revision; complete chains keep their floor. Affected
/// mirrors need a new checkpoint. Never promotes a mirror or invents lost bytes.
///
/// # Safety
/// Production live/serialized, base readable, output writable, error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_resynchronize_history(
    production: *const PpProduction,
    base: *const PpProtocolBase,
    records: u64,
    encoded_bytes: u64,
    out_floor: *mut PpExchangePosition,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output cleared and source decision copied before recovery.
    unsafe {
        initialize_value(out_floor, PpExchangePosition::empty());
        error::call(out_error, || {
            require_output(out_floor, "out_floor")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let base = base
                .as_ref()
                .ok_or_else(|| invalid_argument("source decision must not be null"))?;
            let base = ProtocolBase::try_from(*base)?;
            let limits = ResynchronizationLimits::new(records, encoded_bytes)?;
            let mut store = idle_store(production)?;
            if store.exchange_role() != StoreRole::Authority {
                return Err(ProtocolError::new(
                    FailureKind::MirrorReadOnly,
                    "passive mirror cannot resynchronize history",
                )
                .into());
            }
            out_floor.write(store.resynchronize_exchange_history(base, limits)?.into());
            Ok(())
        })
    }
}
