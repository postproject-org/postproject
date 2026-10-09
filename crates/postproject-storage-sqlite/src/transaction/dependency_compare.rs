use postproject_core::{Dependency, Error, ErrorKind, RepresentationId, Result};
use rusqlite::{Connection, OptionalExtension};

use crate::{decode_dependency, sqlite_error};

// Comparison is part of a write operation, independent of the public collection
// read budget. Retain only one stored occurrence while borrowing caller values.
pub(super) fn current_matches(
    connection: &Connection,
    source: RepresentationId,
    expected: &[Dependency],
) -> Result<bool> {
    let header = connection.query_row(
        "SELECT recorded_revision_sequence, needs_extraction FROM dependency_sets WHERE source_representation_id = ?1",
        [source.as_bytes().as_slice()],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    ).optional().map_err(sqlite_error("compare dependency observation"))?;
    let Some((revision, needs_extraction)) = header else {
        return Ok(false);
    };
    if revision <= 0 || !matches!(needs_extraction, 0 | 1) {
        return Err(Error::new(
            ErrorKind::Storage,
            "stored dependency observation is invalid",
        ));
    }
    if needs_extraction == 1 {
        return Ok(false);
    }
    let mut statement = connection.prepare(
        "SELECT position, source_resource_id, kind, target_kind, target_id, resolved_representation_id, required, authored_reference FROM dependencies WHERE source_representation_id = ?1 ORDER BY position",
    ).map_err(sqlite_error("prepare dependency comparison"))?;
    let mut rows = statement
        .query([source.as_bytes().as_slice()])
        .map_err(sqlite_error("query dependency comparison"))?;
    for (position, expected) in expected.iter().enumerate() {
        let Some(row) = rows
            .next()
            .map_err(sqlite_error("read dependency comparison"))?
        else {
            return Ok(false);
        };
        let stored_position: i64 = row
            .get(0)
            .map_err(sqlite_error("read dependency position"))?;
        if usize::try_from(stored_position).ok() != Some(position) {
            return Err(Error::new(
                ErrorKind::Storage,
                "stored dependency order is invalid",
            ));
        }
        let read = || -> rusqlite::Result<_> {
            Ok((
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        };
        let (resource, kind, target_kind, target, resolved, required, authored) =
            read().map_err(sqlite_error("read dependency occurrence"))?;
        if decode_dependency(
            resource,
            kind,
            target_kind,
            target,
            resolved,
            required,
            authored,
        )? != *expected
        {
            return Ok(false);
        }
    }
    Ok(rows
        .next()
        .map_err(sqlite_error("finish dependency comparison"))?
        .is_none())
}
