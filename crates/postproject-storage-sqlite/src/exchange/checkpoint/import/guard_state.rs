//! Observation history validates declared keys; retained effects additionally
//! establish the exact post-floor set. Initial import locators have no guard.

mod key;
#[cfg(test)]
mod tests;

use postproject_core::{RevisionEvent, RevisionEventKind, SemanticConflictKey};
use rusqlite::{Connection, params};

use crate::{ExchangeResult, sqlite_error, transaction::encode_conflict_key};

use super::super::invalid;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_guard_observations (conflict_key BLOB PRIMARY KEY, sequence INTEGER NOT NULL, required INTEGER NOT NULL); CREATE TABLE checkpoint_guard_effects (conflict_key BLOB PRIMARY KEY, sequence INTEGER NOT NULL);")
        .map_err(sqlite_error("create private semantic guard expectations"))?;
    Ok(())
}

pub(super) fn observed(
    connection: &Connection,
    event: &RevisionEvent,
    sequence: u64,
) -> ExchangeResult<()> {
    let Some(key) = key::from_event(event.kind())? else {
        return Ok(());
    };
    let mut required = true;
    if let RevisionEventKind::LocatorAdded { resource_id, .. } = event.kind() {
        let created: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM revision_events WHERE revision_id = ?1 AND kind = 3 AND primary_id = ?2)", params![event.revision_id().as_bytes().as_slice(), resource_id.as_bytes().as_slice()], |row| row.get(0))
            .map_err(sqlite_error("classify initial locator observation"))?;
        required = !created;
    }
    let changed = connection.execute("INSERT INTO checkpoint_guard_observations (conflict_key, sequence, required) VALUES (?1, ?2, ?3) ON CONFLICT(conflict_key) DO UPDATE SET sequence = excluded.sequence, required = max(required, excluded.required) WHERE sequence <= excluded.sequence", params![encode_conflict_key(&key)?, checked_sequence(sequence)?, required])
        .map_err(sqlite_error("stage observed semantic boundary"))?;
    if changed != 1 {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn recorded(
    connection: &Connection,
    key: &SemanticConflictKey,
    sequence: u64,
) -> ExchangeResult<()> {
    let changed = connection.execute("INSERT INTO checkpoint_guard_effects (conflict_key, sequence) VALUES (?1, ?2) ON CONFLICT(conflict_key) DO UPDATE SET sequence = excluded.sequence WHERE sequence <= excluded.sequence", params![encode_conflict_key(key)?, checked_sequence(sequence)?])
        .map_err(sqlite_error("stage authored semantic boundary"))?;
    if changed != 1 {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn finish(connection: &Connection, floor: u64) -> ExchangeResult<()> {
    let baseline: i64 = connection.query_row("SELECT coalesce((SELECT revision_sequence FROM conflict_migration_baseline WHERE singleton = 1), 0)", [], |row| row.get(0))
        .map_err(sqlite_error("read original conflict baseline"))?;
    let invalid_declared: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM conflict_versions c LEFT JOIN checkpoint_guard_observations o ON o.conflict_key = c.conflict_key WHERE o.conflict_key IS NULL OR c.last_changed_revision_sequence != o.sequence OR c.last_changed_revision_sequence <= ?1)", [baseline], |row| row.get(0))
        .map_err(sqlite_error("validate declared semantic boundaries"))?;
    let missing_observed: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_guard_observations o LEFT JOIN conflict_versions c ON c.conflict_key = o.conflict_key WHERE o.required = 1 AND o.sequence > ?1 AND c.conflict_key IS NULL)", [baseline], |row| row.get(0))
        .map_err(sqlite_error("validate required observed semantic guards"))?;
    let missing_authored: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_guard_effects e LEFT JOIN conflict_versions c ON c.conflict_key = e.conflict_key WHERE c.conflict_key IS NULL OR c.last_changed_revision_sequence != e.sequence)", [], |row| row.get(0))
        .map_err(sqlite_error("validate authored semantic guards"))?;
    let extra_authored: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM conflict_versions c LEFT JOIN checkpoint_guard_effects e ON e.conflict_key = c.conflict_key WHERE c.last_changed_revision_sequence > ?1 AND (e.conflict_key IS NULL OR c.last_changed_revision_sequence != e.sequence))", [i64::try_from(floor).map_err(|_| invalid())?], |row| row.get(0))
        .map_err(sqlite_error("validate complete post-floor semantic guards"))?;
    if invalid_declared || missing_observed || missing_authored || extra_authored {
        return Err(invalid().into());
    }
    connection
        .execute_batch(
            "DROP TABLE checkpoint_guard_effects; DROP TABLE checkpoint_guard_observations;",
        )
        .map_err(sqlite_error("discard private semantic guard expectations"))?;
    Ok(())
}

fn checked_sequence(sequence: u64) -> ExchangeResult<i64> {
    let sequence = i64::try_from(sequence).map_err(|_| invalid())?;
    if sequence <= 0 {
        return Err(invalid().into());
    }
    Ok(sequence)
}
