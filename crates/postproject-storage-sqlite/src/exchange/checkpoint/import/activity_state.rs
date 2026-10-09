//! Indexed immutable frame hashes are private, bounded validation state.

use postproject_core::{ActivityId, ObjectRef};
use postproject_protocol::Document;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    ExchangeResult,
    exchange::{activity_capture, checkpoint::activity_facts},
    sqlite_error,
};

use super::super::invalid;

pub(super) fn create(
    connection: &Connection,
    frames: u64,
    mut check_disk: impl FnMut() -> ExchangeResult<()>,
) -> ExchangeResult<()> {
    connection.execute_batch("CREATE TABLE checkpoint_activity_frames (activity BLOB NOT NULL, position INTEGER NOT NULL, digest BLOB NOT NULL, PRIMARY KEY(activity, position)); CREATE TABLE checkpoint_activity_seen (activity BLOB PRIMARY KEY);")
        .map_err(sqlite_error("create private activity expectations"))?;
    let mut statement = connection
        .prepare("SELECT id FROM activities ORDER BY id")
        .map_err(sqlite_error("prepare immutable activity expectations"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query immutable activity expectations"))?;
    let mut remaining = frames;
    let mut batch = 0;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read immutable activity expectation"))?
    {
        let id = ActivityId::from_bytes(crate::id_bytes(
            row.get(0)
                .map_err(sqlite_error("read expected activity identity"))?,
            "expected activity",
        )?);
        let header = activity_facts::header(connection, id)?;
        let mut position = 0_i64;
        activity_capture::write(connection, &header, &mut |frame| -> ExchangeResult<()> {
            remaining = remaining.checked_sub(1).ok_or_else(super::limits::budget)?;
            connection
                .execute(
                    "INSERT INTO checkpoint_activity_frames VALUES (?1, ?2, ?3)",
                    params![
                        id.as_bytes().as_slice(),
                        position,
                        blake3::hash(&frame.canonical_bytes()?)
                            .as_bytes()
                            .as_slice()
                    ],
                )
                .map_err(sqlite_error("index immutable activity frame"))?;
            position = position.checked_add(1).ok_or_else(invalid)?;
            batch += 1;
            if batch == 1024 {
                check_disk()?;
                batch = 0;
            }
            Ok(())
        })?;
    }
    check_disk()
}

pub(super) fn frame(
    connection: &Connection,
    id: ActivityId,
    position: u64,
    document: &Document,
) -> ExchangeResult<bool> {
    let position = i64::try_from(position).map_err(|_| invalid())?;
    let digest: Option<Vec<u8>> = connection
        .query_row(
            "SELECT digest FROM checkpoint_activity_frames WHERE activity = ?1 AND position = ?2",
            params![id.as_bytes().as_slice(), position],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error("check retained immutable activity frame"))?;
    if digest.as_deref() != Some(blake3::hash(&document.canonical_bytes()?).as_bytes()) {
        return Err(invalid().into());
    }
    let next: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM checkpoint_activity_frames WHERE activity = ?1 AND position = ?2)", params![id.as_bytes().as_slice(), position.checked_add(1).ok_or_else(invalid)?], |row|row.get(0))
        .map_err(sqlite_error("check remaining immutable activity frames"))?;
    Ok(!next)
}

pub(super) fn created(connection: &Connection, id: ActivityId) -> ExchangeResult<()> {
    let inserted = connection
        .execute(
            "INSERT OR IGNORE INTO checkpoint_activity_seen VALUES (?1)",
            [id.as_bytes().as_slice()],
        )
        .map_err(sqlite_error("mark authored activity creation"))?;
    if inserted != 1 {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn require(
    connection: &Connection,
    target: ObjectRef,
    floor: u64,
) -> ExchangeResult<()> {
    let ObjectRef::Activity(id) = target else {
        return Ok(());
    };
    let allowed: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM activities a WHERE a.id = ?1 AND (EXISTS(SELECT 1 FROM checkpoint_activity_seen s WHERE s.activity = a.id) OR (?2 > 0 AND NOT EXISTS(SELECT 1 FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind = ?3 AND e.primary_id = a.id AND r.sequence > ?2))))", params![id.as_bytes().as_slice(), i64::try_from(floor).map_err(|_|invalid())?, crate::stored_revision_event_kind(postproject_core::RevisionEventType::ActivityCreated)?], |row|row.get(0))
        .map_err(sqlite_error("check activity before retained reference"))?;
    if !allowed {
        return Err(invalid().into());
    }
    Ok(())
}

pub(super) fn finish(connection: &Connection, floor: u64) -> ExchangeResult<()> {
    let missing: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM activities a WHERE NOT EXISTS(SELECT 1 FROM checkpoint_activity_seen s WHERE s.activity = a.id) AND (?1 = 0 OR EXISTS(SELECT 1 FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind = ?2 AND e.primary_id = a.id AND r.sequence > ?1)))", params![i64::try_from(floor).map_err(|_|invalid())?, crate::stored_revision_event_kind(postproject_core::RevisionEventType::ActivityCreated)?], |row|row.get(0))
        .map_err(sqlite_error("validate complete retained activity creation"))?;
    if missing {
        return Err(invalid().into());
    }
    connection
        .execute_batch(
            "DROP TABLE checkpoint_activity_seen; DROP TABLE checkpoint_activity_frames;",
        )
        .map_err(sqlite_error("discard private activity expectations"))?;
    Ok(())
}
