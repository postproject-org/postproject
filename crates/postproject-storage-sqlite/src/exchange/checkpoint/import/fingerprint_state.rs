//! Retained transitions must explain exact archive positions and final evidence.

mod lookup;
#[cfg(test)]
mod tests;

use postproject_core::FingerprintSnapshot;
use postproject_protocol::{FingerprintChangeStart, FingerprintObservation, FingerprintState};
use rusqlite::{Connection, params};

use crate::{ExchangeResult, sqlite_error};

use super::super::invalid;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_fingerprint_state (kind INTEGER NOT NULL, owner BLOB NOT NULL, algorithm TEXT NOT NULL, version INTEGER NOT NULL, value BLOB NOT NULL, observed INTEGER, next_archive INTEGER NOT NULL, PRIMARY KEY(kind, owner, algorithm, version)); CREATE TABLE checkpoint_fingerprint_archives (kind INTEGER NOT NULL, owner BLOB NOT NULL, algorithm TEXT NOT NULL, version INTEGER NOT NULL, position INTEGER NOT NULL, PRIMARY KEY(kind, owner, algorithm, version, position)); CREATE TABLE checkpoint_fingerprint_history AS SELECT 2 AS kind, representation_id AS owner, algorithm, algorithm_version AS version, value, observed_revision_sequence AS observed, superseded_revision_sequence AS superseded, row_number() OVER (PARTITION BY representation_id, algorithm, algorithm_version ORDER BY id) - 1 AS position FROM representation_fingerprint_history UNION ALL SELECT 3, resource_id, algorithm, algorithm_version, value, observed_revision_sequence, superseded_revision_sequence, row_number() OVER (PARTITION BY resource_id, algorithm, algorithm_version ORDER BY id) - 1 FROM resource_fingerprint_history; CREATE UNIQUE INDEX checkpoint_fingerprint_history_position ON checkpoint_fingerprint_history(kind, owner, algorithm, version, position); CREATE INDEX checkpoint_fingerprint_history_boundary ON checkpoint_fingerprint_history(kind, owner, algorithm, version, superseded, position);")
        .map_err(sqlite_error("index private checkpoint fingerprint evidence"))?;
    Ok(())
}

pub(super) fn initial(
    connection: &Connection,
    fact: &FingerprintObservation,
) -> ExchangeResult<()> {
    if fact.state() != FingerprintState::Current
        || fact.snapshot().observed_revision_sequence().is_none()
    {
        return Err(invalid().into());
    }
    let domain = lookup::Domain::new(fact.target(), fact.snapshot())?;
    let earliest = lookup::history(connection, &domain, 0)?
        .map(|(snapshot, _)| snapshot)
        .or(lookup::current(connection, &domain)?);
    if earliest.as_ref() != Some(fact.snapshot())
        || lookup::expected(connection, &domain)?.is_some()
    {
        return Err(invalid().into());
    }
    store(connection, &domain, fact.snapshot(), 0)
}

pub(super) fn changed(
    connection: &Connection,
    start: &FingerprintChangeStart,
    sequence: u64,
    floor: u64,
) -> ExchangeResult<()> {
    let domain = lookup::Domain::new(start.target(), start.current())?;
    let (previous, mut next) = match lookup::expected(connection, &domain)? {
        Some((snapshot, next)) => (Some(snapshot), next),
        None if floor == 0 => (None, 0),
        None => lookup::baseline(connection, &domain, floor)?,
    };
    let bytes_changed = previous
        .as_ref()
        .is_none_or(|old| old.value() != start.current().value());
    if previous.as_ref() != start.previous()
        || bytes_changed && start.current().observed_revision_sequence() != Some(sequence)
        || sequence <= floor
    {
        return Err(invalid().into());
    }
    if let Some(position) = start.archived_position() {
        if position != next
            || lookup::history(connection, &domain, position)?
                != previous.clone().map(|snapshot| (snapshot, sequence))
        {
            return Err(invalid().into());
        }
        connection.execute("INSERT INTO checkpoint_fingerprint_archives (kind, owner, algorithm, version, position) VALUES (?1, ?2, ?3, ?4, ?5)", params![domain.kind, domain.owner.as_slice(), domain.algorithm, domain.version, i64::try_from(position).map_err(|_| invalid())?])
            .map_err(sqlite_error("mark explained fingerprint archive"))?;
        next = next.checked_add(1).ok_or_else(invalid)?;
    }
    store(connection, &domain, start.current(), next)
}

fn store(
    connection: &Connection,
    domain: &lookup::Domain<'_>,
    snapshot: &FingerprintSnapshot,
    next: u64,
) -> ExchangeResult<()> {
    connection.execute("INSERT INTO checkpoint_fingerprint_state (kind, owner, algorithm, version, value, observed, next_archive) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(kind, owner, algorithm, version) DO UPDATE SET value = excluded.value, observed = excluded.observed, next_archive = excluded.next_archive", params![domain.kind, domain.owner.as_slice(), domain.algorithm, domain.version, snapshot.value(), snapshot.observed_revision_sequence().map(i64::try_from).transpose().map_err(|_| invalid())?, i64::try_from(next).map_err(|_| invalid())?])
        .map_err(sqlite_error("stage authored current fingerprint evidence"))?;
    Ok(())
}

pub(super) fn finish(connection: &Connection, floor: u64) -> ExchangeResult<()> {
    let floor = i64::try_from(floor).map_err(|_| invalid())?;
    for (kind, table, owner) in [
        (2, "representation_fingerprints", "representation_id"),
        (3, "resource_fingerprints", "resource_id"),
    ] {
        let mismatch: bool = connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM checkpoint_fingerprint_state e LEFT JOIN {table} c ON e.owner = c.{owner} AND e.algorithm = c.algorithm AND e.version = c.algorithm_version WHERE e.kind = ?1 AND (c.{owner} IS NULL OR e.value IS NOT c.value OR e.observed IS NOT c.observed_revision_sequence) UNION ALL SELECT 1 FROM {table} c LEFT JOIN checkpoint_fingerprint_state e ON e.kind = ?1 AND e.owner = c.{owner} AND e.algorithm = c.algorithm AND e.version = c.algorithm_version WHERE e.owner IS NULL AND (?2 = 0 OR c.observed_revision_sequence > ?2))"), params![kind, floor], |row| row.get(0))
            .map_err(sqlite_error("validate authored current fingerprint evidence"))?;
        if mismatch {
            return Err(invalid().into());
        }
    }
    let unexplained: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_fingerprint_history h LEFT JOIN checkpoint_fingerprint_archives e ON e.kind = h.kind AND e.owner = h.owner AND e.algorithm = h.algorithm AND e.version = h.version AND e.position = h.position WHERE h.superseded > ?1 AND e.owner IS NULL)", [floor], |row| row.get(0))
        .map_err(sqlite_error("validate complete post-floor fingerprint archives"))?;
    if unexplained {
        return Err(invalid().into());
    }
    connection.execute_batch("DROP TABLE checkpoint_fingerprint_archives; DROP TABLE checkpoint_fingerprint_state; DROP TABLE checkpoint_fingerprint_history;")
        .map_err(sqlite_error("discard private fingerprint expectations"))?;
    Ok(())
}
