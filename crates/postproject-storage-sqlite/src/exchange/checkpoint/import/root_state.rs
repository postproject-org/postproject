//! Validate retained root transitions and their final state without replaying
//! them against today's roots. A migration may start with unknown root facts.

use postproject_core::MediaRoot;
use postproject_protocol::MediaChange;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error};

use super::super::invalid;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_root_state (id BLOB PRIMARY KEY, name TEXT, label TEXT, legacy_uri TEXT, priority INTEGER, enabled INTEGER, removed INTEGER NOT NULL);")
        .map_err(sqlite_error("create private root expectations"))?;
    Ok(())
}

pub(super) fn change(
    connection: &Connection,
    change: &MediaChange,
    genesis: bool,
) -> ExchangeResult<()> {
    let id = match change {
        MediaChange::RootAdded(root) | MediaChange::RootRemovedWithFacts(root) => root.id(),
        MediaChange::RootEnabled { root_id, .. } | MediaChange::RootRemoved(root_id) => *root_id,
        _ => return Err(invalid().into()),
    };
    let previous: Option<(bool, Option<bool>)> = connection
        .query_row(
            "SELECT removed, enabled FROM checkpoint_root_state WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error("read authored root boundary"))?;
    match change {
        MediaChange::RootAdded(root) => {
            // IDs are stable, but the native API permits explicit reuse after
            // removal. A retained live root cannot be added a second time.
            if previous.is_some_and(|(removed, _)| !removed) {
                return Err(invalid().into());
            }
            unique_name(connection, root)?;
            connection.execute("INSERT INTO checkpoint_root_state (id, name, label, legacy_uri, priority, enabled, removed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0) ON CONFLICT(id) DO UPDATE SET name = excluded.name, label = excluded.label, legacy_uri = excluded.legacy_uri, priority = excluded.priority, enabled = excluded.enabled, removed = 0", params![id.as_bytes().as_slice(), root.name(), root.label(), root.legacy_uri(), root.priority(), root.is_enabled()])
                .map_err(sqlite_error("stage authored root addition"))?;
        }
        MediaChange::RootEnabled { enabled, .. } => {
            if previous.is_some_and(|(removed, state)| removed || state == Some(*enabled))
                || previous.is_none() && genesis
            {
                return Err(invalid().into());
            }
            connection.execute("INSERT INTO checkpoint_root_state (id, enabled, removed) VALUES (?1, ?2, 0) ON CONFLICT(id) DO UPDATE SET enabled = excluded.enabled", params![id.as_bytes().as_slice(), enabled])
                .map_err(sqlite_error("stage authored root state"))?;
        }
        MediaChange::RootRemoved(_) | MediaChange::RootRemovedWithFacts(_) => {
            if previous.is_some_and(|(removed, _)| removed) || previous.is_none() && genesis {
                return Err(invalid().into());
            }
            if let MediaChange::RootRemovedWithFacts(root) = change {
                unique_name(connection, root)?;
                let differs: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_root_state WHERE id = ?1 AND ((name IS NOT NULL AND (name IS NOT ?2 OR label IS NOT ?3 OR legacy_uri IS NOT ?4 OR priority IS NOT ?5)) OR (enabled IS NOT NULL AND enabled IS NOT ?6)))", params![id.as_bytes().as_slice(), root.name(), root.label(), root.legacy_uri(), root.priority(), root.is_enabled()], |row| row.get(0))
                    .map_err(sqlite_error("validate removed root configuration"))?;
                if differs {
                    return Err(invalid().into());
                }
                connection.execute("INSERT INTO checkpoint_root_state (id, name, label, legacy_uri, priority, enabled, removed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1) ON CONFLICT(id) DO UPDATE SET name = excluded.name, label = excluded.label, legacy_uri = excluded.legacy_uri, priority = excluded.priority, enabled = excluded.enabled, removed = 1", params![id.as_bytes().as_slice(), root.name(), root.label(), root.legacy_uri(), root.priority(), root.is_enabled()])
                    .map_err(sqlite_error("stage removed root configuration"))?;
            } else {
                connection.execute("INSERT INTO checkpoint_root_state (id, removed) VALUES (?1, 1) ON CONFLICT(id) DO UPDATE SET removed = 1", [id.as_bytes().as_slice()])
                    .map_err(sqlite_error("stage authored root removal"))?;
            }
        }
        _ => return Err(invalid().into()),
    }
    Ok(())
}

fn unique_name(connection: &Connection, root: &MediaRoot) -> ExchangeResult<()> {
    let duplicate: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_root_state WHERE name = ?1 AND id <> ?2 AND removed = 0)", params![root.name(), root.id().as_bytes().as_slice()], |row| row.get(0))
        .map_err(sqlite_error("check authored root name ownership"))?;
    if duplicate {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn require_name(
    connection: &Connection,
    name: &str,
    genesis: bool,
) -> ExchangeResult<()> {
    let (live, removed): (bool, bool) = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_root_state WHERE name = ?1 AND removed = 0), EXISTS(SELECT 1 FROM checkpoint_root_state WHERE name = ?1 AND removed = 1)", [name], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(sqlite_error("check root association before locator addition"))?;
    if !live && (genesis || removed) {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn finish(connection: &Connection, genesis: bool) -> ExchangeResult<()> {
    let mismatch: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_root_state s LEFT JOIN media_roots r ON r.id = s.id WHERE (s.removed = 1 AND r.id IS NOT NULL) OR (s.removed = 0 AND (r.id IS NULL OR s.enabled IS NOT r.enabled OR (s.name IS NOT NULL AND (s.name IS NOT r.name OR s.label IS NOT r.label OR s.legacy_uri IS NOT r.legacy_uri OR s.priority IS NOT r.priority)))))", [], |row| row.get(0))
        .map_err(sqlite_error("validate authored/current roots"))?;
    let unexplained: bool = genesis && connection.query_row("SELECT EXISTS(SELECT 1 FROM media_roots r LEFT JOIN checkpoint_root_state s ON s.id = r.id WHERE s.id IS NULL)", [], |row| row.get(0))
        .map_err(sqlite_error("validate genesis root origin"))?;
    if mismatch || unexplained {
        return Err(invalid().into());
    }
    connection
        .execute_batch("DROP TABLE checkpoint_root_state;")
        .map_err(sqlite_error("discard private root expectations"))?;
    Ok(())
}
