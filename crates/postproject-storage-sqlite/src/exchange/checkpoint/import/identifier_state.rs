//! Check exact attachment presence and per-target insertion order. Unknown
//! migration attachments can form a prefix; retained additions form its suffix.

use postproject_protocol::IdentifierChange;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error, transaction::encode_conflict_key};

use super::super::invalid;

#[cfg(test)]
mod tests;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_identifier_state (ordinal INTEGER PRIMARY KEY, conflict_key BLOB NOT NULL UNIQUE, target_kind INTEGER NOT NULL, target_id BLOB NOT NULL, scheme TEXT NOT NULL, value TEXT NOT NULL, qualifier TEXT, present INTEGER NOT NULL);")
        .map_err(sqlite_error("create private identifier expectations"))?;
    Ok(())
}

pub(super) fn change(
    connection: &Connection,
    change: &IdentifierChange,
    genesis: bool,
) -> ExchangeResult<()> {
    let attachment = change.attachment();
    let target = attachment.target();
    let (kind, id) = crate::encode_metadata_target(&target)?;
    let key = encode_conflict_key(&change.conflict_key())?;
    let previous: Option<bool> = connection
        .query_row(
            "SELECT present FROM checkpoint_identifier_state WHERE conflict_key = ?1",
            [&key],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error("read authored identifier presence"))?;
    let present = matches!(change, IdentifierChange::Added(_));
    if previous == Some(present) || !present && previous.is_none() && genesis {
        return Err(invalid().into());
    }
    let identifier = attachment.identifier();
    connection.execute("INSERT INTO checkpoint_identifier_state (conflict_key, target_kind, target_id, scheme, value, qualifier, present) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(conflict_key) DO UPDATE SET ordinal = excluded.ordinal, present = excluded.present", params![key, kind, id.as_slice(), identifier.scheme().as_str(), identifier.value(), identifier.qualifier(), present])
        .map_err(sqlite_error("stage authored identifier change"))?;
    Ok(())
}

pub(super) fn finish(connection: &Connection, genesis: bool) -> ExchangeResult<()> {
    let join = "a.target_kind = e.target_kind AND a.target_id = e.target_id AND a.scheme = e.scheme AND a.value = e.value AND a.qualifier IS e.qualifier";
    let wrong_presence: bool = connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM checkpoint_identifier_state e LEFT JOIN external_identifiers a ON {join} WHERE e.present != (a.id IS NOT NULL))"), [], |row| row.get(0))
        .map_err(sqlite_error("validate authored identifier presence"))?;
    let wrong_order: bool = connection.query_row(&format!("WITH ordered AS (SELECT e.ordinal, max(e.ordinal) OVER (PARTITION BY a.target_kind, a.target_id ORDER BY a.id ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING) AS previous FROM external_identifiers a LEFT JOIN checkpoint_identifier_state e ON {join}) SELECT EXISTS(SELECT 1 FROM ordered WHERE previous IS NOT NULL AND (ordinal IS NULL OR ordinal <= previous))"), [], |row| row.get(0))
        .map_err(sqlite_error("validate authored identifier insertion order"))?;
    let unexplained: bool = genesis && connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM external_identifiers a LEFT JOIN checkpoint_identifier_state e ON {join} WHERE e.ordinal IS NULL)"), [], |row| row.get(0))
        .map_err(sqlite_error("validate genesis identifier origin"))?;
    if wrong_presence || wrong_order || unexplained {
        return Err(invalid().into());
    }
    connection
        .execute_batch("DROP TABLE checkpoint_identifier_state;")
        .map_err(sqlite_error("discard private identifier expectations"))?;
    Ok(())
}
