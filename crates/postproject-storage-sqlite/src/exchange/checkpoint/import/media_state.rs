//! Immutable creations and the last authored resource measurements are checked
//! against current state. A migration floor permits unknown earlier creations.

use postproject_core::{Asset, ContentStructure, FileFacts, RepresentationId, ResourceId};
use postproject_protocol::{RepresentationHeader, ResourceHeader};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error};

use super::super::{invalid, media_facts};

mod presence;
#[cfg(test)]
mod tests;

pub(super) use presence::require;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_media_created (kind INTEGER NOT NULL, id BLOB NOT NULL, PRIMARY KEY(kind, id)); CREATE TABLE checkpoint_resource_facts (id BLOB PRIMARY KEY, size INTEGER, modified INTEGER);")
        .map_err(sqlite_error("create private media expectations"))?;
    Ok(())
}

pub(super) fn asset(connection: &Connection, asset: &Asset) -> ExchangeResult<()> {
    created(connection, 1, asset.id().as_bytes())?;
    if media_facts::asset(connection, asset.id())? != *asset {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn representation(
    connection: &Connection,
    header: RepresentationHeader,
) -> ExchangeResult<()> {
    created(connection, 2, header.id().as_bytes())?;
    if media_facts::representation(connection, header.id())? != header {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn structure(
    connection: &Connection,
    owner: RepresentationId,
    structure: &ContentStructure,
) -> ExchangeResult<()> {
    if media_facts::content(connection, owner)? != *structure {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn resource(connection: &Connection, header: ResourceHeader) -> ExchangeResult<()> {
    created(connection, 3, header.id().as_bytes())?;
    facts(connection, header.id(), header.file_facts(), true, false)
}

pub(super) fn changed_facts(
    connection: &Connection,
    id: ResourceId,
    value: FileFacts,
    genesis: bool,
) -> ExchangeResult<()> {
    facts(connection, id, Some(value), false, genesis)
}

fn facts(
    connection: &Connection,
    id: ResourceId,
    value: Option<FileFacts>,
    initial: bool,
    genesis: bool,
) -> ExchangeResult<()> {
    let size = value
        .map(|value| i64::try_from(value.size_bytes()).map_err(|_| invalid()))
        .transpose()?;
    let modified = value.and_then(|value| {
        value
            .modified_at()
            .map(postproject_core::Timestamp::as_unix_micros)
    });
    let previous: Option<(Option<i64>, Option<i64>)> = connection
        .query_row(
            "SELECT size, modified FROM checkpoint_resource_facts WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error("read authored resource measurements"))?;
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM resources WHERE id = ?1)",
            [id.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check measured resource ownership"))?;
    if !exists
        || initial && previous.is_some()
        || !initial && (previous == Some((size, modified)) || genesis && previous.is_none())
    {
        return Err(invalid().into());
    }
    connection.execute("INSERT INTO checkpoint_resource_facts (id, size, modified) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET size = excluded.size, modified = excluded.modified", params![id.as_bytes().as_slice(), size, modified])
        .map_err(sqlite_error("stage authored resource measurements"))?;
    Ok(())
}

fn created(connection: &Connection, kind: i64, id: &[u8; 16]) -> ExchangeResult<()> {
    let table = match kind {
        1 => "assets",
        2 => "representations",
        3 => "resources",
        _ => return Err(invalid().into()),
    };
    let exists: bool = connection
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
            [id.as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check checkpoint creation identity"))?;
    if !exists {
        return Err(invalid().into());
    }
    connection
        .execute(
            "INSERT INTO checkpoint_media_created (kind, id) VALUES (?1, ?2)",
            params![kind, id.as_slice()],
        )
        .map_err(|error| {
            if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
                crate::ExchangeError::Protocol(invalid())
            } else {
                sqlite_error("stage unique media creation")(error).into()
            }
        })?;
    Ok(())
}

pub(super) fn finish(connection: &Connection, genesis: bool) -> ExchangeResult<()> {
    let changed: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_resource_facts e LEFT JOIN resources r ON r.id = e.id WHERE r.id IS NULL OR r.file_size_bytes IS NOT e.size OR r.modified_at_micros IS NOT e.modified)", [], |row| row.get(0))
        .map_err(sqlite_error("validate last authored resource measurements"))?;
    if changed {
        return Err(invalid().into());
    }
    if genesis {
        for (kind, table) in [(1, "assets"), (2, "representations"), (3, "resources")] {
            let unexplained: bool = connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} a LEFT JOIN checkpoint_media_created e ON e.kind = ?1 AND e.id = a.id WHERE e.id IS NULL)"), [kind], |row| row.get(0))
                .map_err(sqlite_error("validate genesis media origin"))?;
            if unexplained {
                return Err(invalid().into());
            }
        }
    }
    connection
        .execute_batch("DROP TABLE checkpoint_resource_facts; DROP TABLE checkpoint_media_created;")
        .map_err(sqlite_error("discard private media expectations"))?;
    Ok(())
}
