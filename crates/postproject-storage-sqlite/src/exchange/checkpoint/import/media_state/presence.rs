//! Final existence does not authorize a reference before its creation effect.

use postproject_core::ObjectRef;
use rusqlite::{Connection, params};

use crate::{ExchangeResult, sqlite_error};

use super::super::super::invalid;

pub(in crate::exchange::checkpoint::import) fn require(
    connection: &Connection,
    target: ObjectRef,
    floor: u64,
) -> ExchangeResult<()> {
    let (kind, id, table) = match target {
        ObjectRef::Asset(id) => (1, id.into_bytes(), "assets"),
        ObjectRef::Representation(id) => (2, id.into_bytes(), "representations"),
        ObjectRef::Resource(id) => (3, id.into_bytes(), "resources"),
        _ => return Err(invalid().into()),
    };
    let exists: bool = connection
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
            [id.as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check referenced checkpoint media"))?;
    let authored: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM checkpoint_media_created WHERE kind = ?1 AND id = ?2)",
            params![kind, id.as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error(
            "check media creation before retained reference",
        ))?;
    if !exists {
        return Err(invalid().into());
    }
    if !authored {
        // A migration permits unknown baseline creation facts. It cannot hide
        // an observed later creation behind the final-state object's presence.
        let later: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind = ?1 AND e.primary_id = ?2 AND r.sequence > ?3)",
            params![kind, id.as_slice(), i64::try_from(floor).map_err(|_| invalid())?],
            |row| row.get(0),
        ).map_err(sqlite_error("check media creation above migration floor"))?;
        if floor == 0 || later {
            return Err(invalid().into());
        }
    }
    Ok(())
}
