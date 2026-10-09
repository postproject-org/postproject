//! Recorded dirty-marker transitions preserve their original resource and time.

use postproject_core::{ObjectRef, RepresentationId, ResourceId};
use postproject_protocol::{FingerprintChangeStart, FingerprintRecomputation};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error};

use super::super::invalid;

#[cfg(test)]
mod tests;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_recomputation_state (representation BLOB PRIMARY KEY, resource BLOB, sequence INTEGER, cleared INTEGER NOT NULL); CREATE TABLE checkpoint_recomputation_members (representation BLOB PRIMARY KEY);")
        .map_err(sqlite_error("create private recomputation expectations"))?;
    Ok(())
}

pub(super) fn born(connection: &Connection, id: RepresentationId) -> ExchangeResult<()> {
    connection
        .execute(
            "INSERT INTO checkpoint_recomputation_state (representation, cleared) VALUES (?1, 1)",
            [id.as_bytes().as_slice()],
        )
        .map_err(sqlite_error("stage clean representation creation"))?;
    Ok(())
}

pub(super) fn resource_start(
    connection: &Connection,
    resource: ResourceId,
    count: u64,
) -> ExchangeResult<()> {
    let owners: i64 = connection.query_row("SELECT count(DISTINCT representation_id) FROM representation_resources WHERE resource_id = ?1", [resource.as_bytes().as_slice()], |row| row.get(0))
        .map_err(sqlite_error("validate recorded recomputation owner count"))?;
    if u64::try_from(owners).ok() != Some(count) {
        return Err(invalid().into());
    }
    connection
        .execute("DELETE FROM checkpoint_recomputation_members", [])
        .map_err(sqlite_error("start recorded recomputation membership"))?;
    Ok(())
}

pub(super) fn marked(
    connection: &Connection,
    marker: FingerprintRecomputation,
    resource: ResourceId,
    sequence: u64,
) -> ExchangeResult<()> {
    if marker.changed_resource_id() != resource
        || marker.revision_sequence() != sequence
        || !member(connection, marker)?
    {
        return Err(invalid().into());
    }
    let duplicate: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_recomputation_members WHERE representation = ?1)", [marker.representation_id().as_bytes().as_slice()], |row| row.get(0))
        .map_err(sqlite_error("check unique recomputation owner"))?;
    if duplicate {
        return Err(invalid().into());
    }
    connection
        .execute(
            "INSERT INTO checkpoint_recomputation_members (representation) VALUES (?1)",
            [marker.representation_id().as_bytes().as_slice()],
        )
        .map_err(sqlite_error("stage complete recomputation membership"))?;
    connection.execute("INSERT INTO checkpoint_recomputation_state (representation, resource, sequence, cleared) VALUES (?1, ?2, ?3, 0) ON CONFLICT(representation) DO UPDATE SET resource = excluded.resource, sequence = excluded.sequence, cleared = 0", params![marker.representation_id().as_bytes().as_slice(), resource.as_bytes().as_slice(), i64::try_from(sequence).map_err(|_| invalid())?])
        .map_err(sqlite_error("stage original recomputation evidence"))?;
    Ok(())
}

pub(super) fn resource_finish(connection: &Connection, count: u64) -> ExchangeResult<()> {
    let actual: i64 = connection
        .query_row(
            "SELECT count(*) FROM checkpoint_recomputation_members",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check complete recomputation membership"))?;
    if u64::try_from(actual).ok() != Some(count) {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn cleared(
    connection: &Connection,
    start: &FingerprintChangeStart,
    sequence: u64,
    floor: u64,
) -> ExchangeResult<()> {
    let ObjectRef::Representation(id) = start.target() else {
        return Err(invalid().into());
    };
    let previous: Option<(Option<Vec<u8>>, Option<i64>, bool)> = connection.query_row("SELECT resource, sequence, cleared FROM checkpoint_recomputation_state WHERE representation = ?1", [id.as_bytes().as_slice()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .optional().map_err(sqlite_error("read authored dirty-marker boundary"))?;
    let marker = start.cleared_marker();
    if let Some((resource, boundary, cleared)) = previous {
        if cleared != marker.is_none()
            || marker.is_some_and(|marker| {
                resource.as_deref() != Some(marker.changed_resource_id().as_bytes().as_slice())
                    || boundary.and_then(|sequence| u64::try_from(sequence).ok())
                        != Some(marker.revision_sequence())
            })
        {
            return Err(invalid().into());
        }
    } else if marker.is_some_and(|marker| marker.revision_sequence() > floor) {
        return Err(invalid().into());
    }
    if let Some(marker) = marker {
        if marker.revision_sequence() > sequence || !member(connection, marker)? {
            return Err(invalid().into());
        }
    }
    connection.execute("INSERT INTO checkpoint_recomputation_state (representation, cleared) VALUES (?1, 1) ON CONFLICT(representation) DO UPDATE SET resource = NULL, sequence = NULL, cleared = 1", [id.as_bytes().as_slice()])
        .map_err(sqlite_error("stage original recomputation clearing"))?;
    Ok(())
}

fn member(connection: &Connection, marker: FingerprintRecomputation) -> ExchangeResult<bool> {
    Ok(connection.query_row("SELECT EXISTS(SELECT 1 FROM representation_resources WHERE representation_id = ?1 AND resource_id = ?2)", params![marker.representation_id().as_bytes().as_slice(), marker.changed_resource_id().as_bytes().as_slice()], |row| row.get(0))
        .map_err(sqlite_error("validate recomputation resource membership"))?)
}

pub(super) fn finish(connection: &Connection, floor: u64) -> ExchangeResult<()> {
    let mismatch: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_recomputation_state e LEFT JOIN representation_fingerprint_recomputations m ON m.representation_id = e.representation WHERE (e.cleared = 1 AND m.representation_id IS NOT NULL) OR (e.cleared = 0 AND (m.representation_id IS NULL OR m.changed_resource_id IS NOT e.resource OR m.marked_revision_sequence IS NOT e.sequence)) UNION ALL SELECT 1 FROM representation_fingerprint_recomputations m LEFT JOIN checkpoint_recomputation_state e ON e.representation = m.representation_id WHERE e.representation IS NULL AND (?1 = 0 OR m.marked_revision_sequence > ?1))", [i64::try_from(floor).map_err(|_| invalid())?], |row| row.get(0))
        .map_err(sqlite_error("validate authored final recomputation evidence"))?;
    if mismatch {
        return Err(invalid().into());
    }
    connection.execute_batch("DROP TABLE checkpoint_recomputation_members; DROP TABLE checkpoint_recomputation_state;")
        .map_err(sqlite_error("discard private recomputation expectations"))?;
    Ok(())
}
