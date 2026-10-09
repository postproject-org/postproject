//! Resume a sealed owned stage, or discard an owned unpublished incomplete one.

use std::path::Path;

use postproject_core::{Error, ErrorKind};
use postproject_protocol::StoreRole;

use crate::{
    CURRENT_SCHEMA_VERSION, ExchangeResult, SqliteProduction, load_production, open_connection,
    sqlite_error,
};

use super::{
    super::invalid,
    CheckpointLimits, completion,
    staging::{io_error, promote},
};

pub(crate) fn recover(
    directory: &Path,
    destination: &Path,
    limits: CheckpointLimits,
) -> ExchangeResult<Option<SqliteProduction>> {
    let stage = completion::inspect(directory, limits.disk_bytes)?;
    if !stage.complete {
        // The validated ownership marker and exact file allowlist are required
        // before deleting anything. Refuse unknown files and all symlinks.
        for entry in stage.entries {
            std::fs::remove_file(entry).map_err(|error| io_error(&error))?;
        }
        std::fs::remove_dir(directory).map_err(|error| io_error(&error))?;
        return Ok(None);
    }
    if destination.try_exists().map_err(|error| io_error(&error))? {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            "checkpoint recovery destination exists",
        )
        .into());
    }
    let connection = open_connection(&stage.file)?;
    let schema: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sqlite_error("validate sealed checkpoint schema"))?;
    if schema != CURRENT_SCHEMA_VERSION {
        return Err(Error::new(
            ErrorKind::Unsupported,
            "sealed checkpoint staging schema differs",
        )
        .into());
    }
    let production = load_production(&connection)?;
    let sequence: i64 = connection
        .query_row(
            "SELECT coalesce(max(sequence), 0) FROM revisions",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error("validate sealed checkpoint head"))?;
    if crate::exchange::role(&connection)? != StoreRole::PassiveMirror
        || production.id() != stage.manifest.head().scope().production()
        || crate::exchange::floor(&connection, production.id())? != stage.manifest.floor()
        || crate::exchange::position(
            &connection,
            production.id(),
            crate::stored_u64(sequence, "sealed checkpoint sequence")?,
        )? != Some(stage.manifest.head())
    {
        return Err(invalid().into());
    }
    connection
        .close()
        .map_err(|(_, error)| sqlite_error("close recovered checkpoint staging")(error))?;
    promote(&stage.file, destination)?;
    for entry in stage.entries {
        if entry != stage.file {
            std::fs::remove_file(entry).map_err(|error| io_error(&error))?;
        }
    }
    std::fs::remove_dir(directory).map_err(|error| io_error(&error))?;
    Ok(Some(SqliteProduction::open(destination)?))
}
