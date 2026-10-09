//! Validate the checkpoint's current assertions against retained authored edits.
//!
//! A first append after a migration floor can have an unknown earlier prefix.
//! Retain that prefix as source baseline facts; a replacement/removal establishes
//! the whole property. Genesis histories have no unknown prefix.

use postproject_protocol::{MetadataEffectStart, MetadataOperation};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error};

use super::super::invalid;

pub(super) fn create(connection: &Connection) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_metadata_state (target_kind INTEGER NOT NULL, target_id BLOB NOT NULL, vocabulary TEXT NOT NULL, property TEXT NOT NULL, prefix INTEGER NOT NULL, total INTEGER NOT NULL, PRIMARY KEY(target_kind, target_id, vocabulary, property)); CREATE TABLE checkpoint_metadata_expected (target_kind INTEGER NOT NULL, target_id BLOB NOT NULL, vocabulary TEXT NOT NULL, property TEXT NOT NULL, position INTEGER NOT NULL, encoded_value BLOB NOT NULL, PRIMARY KEY(target_kind, target_id, vocabulary, property, position));")
        .map_err(sqlite_error("create private checkpoint consistency checks"))?;
    Ok(())
}

pub(super) fn start(
    connection: &Connection,
    effect: &MetadataEffectStart,
    genesis: bool,
) -> ExchangeResult<()> {
    let target = effect.target();
    let (kind, id) = crate::encode_metadata_target(&target)?;
    let property = effect.property();
    let vocabulary = property.vocabulary().as_str();
    let property = property.property().as_str();
    let previous: Option<(i64, i64)> = connection.query_row("SELECT prefix, total FROM checkpoint_metadata_state WHERE target_kind = ?1 AND target_id = ?2 AND vocabulary = ?3 AND property = ?4", params![kind, id, vocabulary, property], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional().map_err(sqlite_error("read private authored metadata boundary"))?;
    let (prefix, total) = match effect.operation() {
        MetadataOperation::Appended(position) => {
            let position = i64::try_from(position).map_err(|_| invalid())?;
            let (prefix, total) = previous.unwrap_or(if genesis {
                (0, 0)
            } else {
                (position, position)
            });
            if total != position {
                return Err(invalid().into());
            }
            (prefix, total.checked_add(1).ok_or_else(invalid)?)
        }
        MetadataOperation::Replaced | MetadataOperation::Removed => {
            // A known empty property cannot be removed by a real source edit.
            if effect.value_count() == 0
                && (previous.is_some_and(|(_, total)| total == 0) || previous.is_none() && genesis)
            {
                return Err(invalid().into());
            }
            connection.execute("DELETE FROM checkpoint_metadata_expected WHERE target_kind = ?1 AND target_id = ?2 AND vocabulary = ?3 AND property = ?4", params![kind, id, vocabulary, property])
                .map_err(sqlite_error("replace private authored metadata expectation"))?;
            (
                0,
                i64::try_from(effect.value_count()).map_err(|_| invalid())?,
            )
        }
    };
    connection.execute("INSERT INTO checkpoint_metadata_state (target_kind, target_id, vocabulary, property, prefix, total) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(target_kind, target_id, vocabulary, property) DO UPDATE SET prefix = excluded.prefix, total = excluded.total", params![kind, id, vocabulary, property, prefix, total])
        .map_err(sqlite_error("stage authored metadata boundary"))?;
    Ok(())
}

pub(super) fn value(
    connection: &Connection,
    effect: &MetadataEffectStart,
    index: u64,
    encoded: &[u8],
) -> ExchangeResult<()> {
    let target = effect.target();
    let (kind, id) = crate::encode_metadata_target(&target)?;
    let property = effect.property();
    let position = match effect.operation() {
        MetadataOperation::Appended(position) => position,
        MetadataOperation::Replaced => index,
        MetadataOperation::Removed => return Err(invalid().into()),
    };
    connection.execute("INSERT INTO checkpoint_metadata_expected (target_kind, target_id, vocabulary, property, position, encoded_value) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![kind, id, property.vocabulary().as_str(), property.property().as_str(), i64::try_from(position).map_err(|_| invalid())?, encoded])
        .map_err(sqlite_error("stage authored metadata value"))?;
    Ok(())
}

pub(super) fn finish(connection: &Connection, genesis: bool) -> ExchangeResult<()> {
    let count_mismatch: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_metadata_state s WHERE s.total != (SELECT count(*) FROM metadata_assertions a WHERE a.target_kind = s.target_kind AND a.target_id = s.target_id AND a.vocabulary = s.vocabulary AND a.property = s.property) OR s.total - s.prefix != (SELECT count(*) FROM checkpoint_metadata_expected e WHERE e.target_kind = s.target_kind AND e.target_id = s.target_id AND e.vocabulary = s.vocabulary AND e.property = s.property))", [], |row| row.get(0))
        .map_err(sqlite_error("validate authored/current metadata counts"))?;
    let value_mismatch: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_metadata_expected e LEFT JOIN metadata_assertions a ON a.target_kind = e.target_kind AND a.target_id = e.target_id AND a.vocabulary = e.vocabulary AND a.property = e.property AND a.position = e.position WHERE a.encoded_value IS NULL OR a.encoded_value != e.encoded_value)", [], |row| row.get(0))
        .map_err(sqlite_error("validate authored/current metadata values"))?;
    if count_mismatch || value_mismatch {
        return Err(invalid().into());
    }
    if genesis {
        let unexplained: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM metadata_assertions a LEFT JOIN checkpoint_metadata_state s ON s.target_kind = a.target_kind AND s.target_id = a.target_id AND s.vocabulary = a.vocabulary AND s.property = a.property WHERE s.target_id IS NULL)", [], |row| row.get(0))
            .map_err(sqlite_error("validate genesis metadata origin"))?;
        if unexplained {
            return Err(invalid().into());
        }
    }
    connection
        .execute_batch(
            "DROP TABLE checkpoint_metadata_expected; DROP TABLE checkpoint_metadata_state;",
        )
        .map_err(sqlite_error(
            "discard private checkpoint consistency checks",
        ))?;
    Ok(())
}
