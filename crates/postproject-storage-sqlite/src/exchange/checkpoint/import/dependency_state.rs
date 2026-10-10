//! Derive mutable dependency observations without inventing pre-floor contents.

mod baseline;
mod occurrence;
mod replacement;
mod snapshots;
#[cfg(test)]
mod tests;
mod walk;

pub(super) use replacement::Replacement;
pub(super) use snapshots::{segment, subject};
pub(super) use walk::validate_paths;

use postproject_core::RepresentationId;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error};

use super::super::invalid;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_dependency_state (owner BLOB PRIMARY KEY NOT NULL, present INTEGER, dirty INTEGER, recorded INTEGER, count INTEGER, baseline INTEGER NOT NULL); CREATE TABLE checkpoint_dependency_occurrences (owner BLOB NOT NULL, position INTEGER NOT NULL, document BLOB NOT NULL, PRIMARY KEY(owner, position));")
        .map_err(sqlite_error("create private dependency expectations"))?;
    Ok(())
}

#[derive(Clone, Copy)]
struct State {
    present: Option<bool>,
    dirty: Option<bool>,
    recorded: Option<i64>,
    count: Option<i64>,
    baseline: bool,
}

fn state(connection: &Connection, owner: RepresentationId) -> ExchangeResult<Option<State>> {
    connection.query_row("SELECT present, dirty, recorded, count, baseline FROM checkpoint_dependency_state WHERE owner = ?1", [owner.as_bytes().as_slice()], |row| Ok(State { present: row.get(0)?, dirty: row.get(1)?, recorded: row.get(2)?, count: row.get(3)?, baseline: row.get(4)? }))
        .optional().map_err(sqlite_error("read authored dependency expectation")).map_err(Into::into)
}

fn store(connection: &Connection, owner: RepresentationId, state: State) -> ExchangeResult<()> {
    connection.execute("INSERT INTO checkpoint_dependency_state (owner, present, dirty, recorded, count, baseline) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(owner) DO UPDATE SET present = excluded.present, dirty = excluded.dirty, recorded = excluded.recorded, count = excluded.count, baseline = excluded.baseline", params![owner.as_bytes().as_slice(), state.present, state.dirty, state.recorded, state.count, state.baseline])
        .map_err(sqlite_error("stage authored dependency expectation"))?;
    Ok(())
}

pub(super) fn invalidated(
    connection: &Connection,
    owner: RepresentationId,
    invalidates: bool,
    floor: u64,
) -> ExchangeResult<()> {
    let mut prior = baseline::ensure(connection, owner, floor)?;
    match (prior.present, prior.dirty) {
        (Some(false), _) | (_, Some(true)) if invalidates => return Err(invalid().into()),
        (Some(true), Some(false)) if !invalidates => return Err(invalid().into()),
        _ => {}
    }
    // Every emitted representation-fingerprint transition leaves any existing
    // set dirty, including transitions whose fingerprint bytes are unchanged.
    if invalidates {
        prior.present = Some(true);
    }
    prior.dirty = Some(true);
    store(connection, owner, prior)
}

pub(super) fn finish(connection: &Connection, floor: u64) -> ExchangeResult<()> {
    let floor = i64::try_from(floor).map_err(|_| invalid())?;
    let mismatch: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_dependency_state e LEFT JOIN dependency_sets c ON c.source_representation_id = e.owner WHERE (e.present = 0 AND c.source_representation_id IS NOT NULL) OR (e.present = 1 AND c.source_representation_id IS NULL) OR (c.source_representation_id IS NOT NULL AND ((e.dirty IS NOT NULL AND e.dirty IS NOT c.needs_extraction) OR (e.recorded IS NOT NULL AND e.recorded IS NOT c.recorded_revision_sequence) OR (e.count IS NOT NULL AND e.count IS NOT (SELECT COUNT(*) FROM dependencies d WHERE d.source_representation_id = e.owner)))) UNION ALL SELECT 1 FROM dependency_sets c LEFT JOIN checkpoint_dependency_state e ON e.owner = c.source_representation_id WHERE c.recorded_revision_sequence > ?1 AND (e.owner IS NULL OR e.recorded IS NOT c.recorded_revision_sequence))", [floor], |row| row.get(0))
        .map_err(sqlite_error("validate final dependency headers"))?;
    if mismatch {
        return Err(invalid().into());
    }
    occurrence::finish(connection)?;
    connection
        .execute_batch(
            "DROP TABLE checkpoint_dependency_occurrences; DROP TABLE checkpoint_dependency_state;",
        )
        .map_err(sqlite_error("discard private dependency expectations"))?;
    Ok(())
}
