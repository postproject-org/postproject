//! Track authored locator additions and retirements independently of final rows.
//! Explicit reuse after retirement is valid; live duplicates are not.

use postproject_core::{
    Locator, LocatorAvailability, LocatorId, ResourceId, SequenceNaming, Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error};

use super::super::invalid;

#[cfg(test)]
mod tests;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_locator_state (id BLOB PRIMARY KEY, resource BLOB NOT NULL, uri TEXT, last_seen INTEGER, availability INTEGER, root TEXT, prefix TEXT, suffix TEXT, padding INTEGER, retired INTEGER NOT NULL);")
        .map_err(sqlite_error("create private locator expectations"))?;
    Ok(())
}

pub(super) fn added(connection: &Connection, locator: &Locator) -> ExchangeResult<()> {
    let previous = previous(connection, locator.id())?;
    if previous.is_some_and(|(_, retired)| !retired) {
        return Err(invalid().into());
    }
    let naming = locator.sequence_naming();
    let sequence_resource: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM image_sequences WHERE resource_id = ?1)",
            [locator.resource_id().as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check authored locator content kind"))?;
    let resource_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM resources WHERE id = ?1)",
            [locator.resource_id().as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check authored locator resource"))?;
    let duplicate: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_locator_state WHERE retired = 0 AND resource = ?1 AND uri = ?2 AND prefix IS ?3 AND suffix IS ?4 AND padding IS ?5)", params![locator.resource_id().as_bytes().as_slice(), locator.uri(), naming.map(SequenceNaming::prefix), naming.map(SequenceNaming::suffix), naming.map(SequenceNaming::padding)], |row| row.get(0))
        .map_err(sqlite_error("check unique authored locator access route"))?;
    if !resource_exists || sequence_resource != naming.is_some() || duplicate {
        return Err(invalid().into());
    }
    let availability = match locator.availability() {
        LocatorAvailability::Unknown => 0,
        LocatorAvailability::Online => 1,
        LocatorAvailability::Offline => 2,
        _ => return Err(invalid().into()),
    };
    connection.execute("INSERT INTO checkpoint_locator_state (id, resource, uri, last_seen, availability, root, prefix, suffix, padding, retired) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0) ON CONFLICT(id) DO UPDATE SET resource = excluded.resource, uri = excluded.uri, last_seen = excluded.last_seen, availability = excluded.availability, root = excluded.root, prefix = excluded.prefix, suffix = excluded.suffix, padding = excluded.padding, retired = 0", params![locator.id().as_bytes().as_slice(), locator.resource_id().as_bytes().as_slice(), locator.uri(), locator.last_seen().map(Timestamp::as_unix_micros), availability, locator.media_root(), naming.map(SequenceNaming::prefix), naming.map(SequenceNaming::suffix), naming.map(SequenceNaming::padding)])
        .map_err(sqlite_error("stage authored locator addition"))?;
    Ok(())
}

pub(super) fn retired(
    connection: &Connection,
    id: LocatorId,
    resource: ResourceId,
    genesis: bool,
) -> ExchangeResult<()> {
    let previous = previous(connection, id)?;
    if previous.is_some_and(|(owner, retired)| owner != resource || retired)
        || previous.is_none() && genesis
    {
        return Err(invalid().into());
    }
    connection.execute("INSERT INTO checkpoint_locator_state (id, resource, retired) VALUES (?1, ?2, 1) ON CONFLICT(id) DO UPDATE SET retired = 1", params![id.as_bytes().as_slice(), resource.as_bytes().as_slice()])
        .map_err(sqlite_error("stage authored locator retirement"))?;
    Ok(())
}

fn previous(connection: &Connection, id: LocatorId) -> ExchangeResult<Option<(ResourceId, bool)>> {
    connection
        .query_row(
            "SELECT resource, retired FROM checkpoint_locator_state WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()
        .map_err(sqlite_error("read authored locator boundary"))?
        .map(|(owner, retired)| {
            Ok((
                ResourceId::from_bytes(crate::id_bytes(owner, "authored locator resource")?),
                retired,
            ))
        })
        .transpose()
}

pub(super) fn clear_known_root(connection: &Connection, name: &str) -> ExchangeResult<()> {
    // Native root deletion clears the association through ON DELETE SET NULL.
    // Readding that root does not reattach earlier locators.
    connection
        .execute(
            "UPDATE checkpoint_locator_state SET root = NULL WHERE root = ?1 AND retired = 0",
            [name],
        )
        .map_err(sqlite_error("stage implicit locator root removal"))?;
    Ok(())
}

pub(super) fn finish(connection: &Connection, genesis: bool) -> ExchangeResult<()> {
    let mismatch: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_locator_state e LEFT JOIN locators l ON l.id = e.id LEFT JOIN locator_sequence_namings n ON n.locator_id = l.id WHERE (e.retired = 1 AND l.id IS NOT NULL) OR (e.retired = 0 AND (l.id IS NULL OR l.resource_id IS NOT e.resource OR l.uri IS NOT e.uri OR l.last_seen_micros IS NOT e.last_seen OR l.availability IS NOT e.availability OR l.media_root_name IS NOT e.root OR n.prefix IS NOT e.prefix OR n.suffix IS NOT e.suffix OR n.padding IS NOT e.padding)))", [], |row| row.get(0))
        .map_err(sqlite_error("validate authored current locator facts"))?;
    let unexplained: bool = genesis && connection.query_row("SELECT EXISTS(SELECT 1 FROM locators l LEFT JOIN checkpoint_locator_state e ON e.id = l.id WHERE e.id IS NULL)", [], |row| row.get(0))
        .map_err(sqlite_error("validate genesis locator origin"))?;
    if mismatch || unexplained {
        return Err(invalid().into());
    }
    connection
        .execute_batch("DROP TABLE checkpoint_locator_state;")
        .map_err(sqlite_error("discard private locator expectations"))?;
    Ok(())
}
