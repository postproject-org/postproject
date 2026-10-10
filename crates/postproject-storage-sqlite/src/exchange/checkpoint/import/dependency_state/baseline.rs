use postproject_core::{ObjectRef, RepresentationId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, exchange::checkpoint::dependency_facts, sqlite_error};

use super::{State, invalid, state, store};

pub(super) fn ensure(
    connection: &Connection,
    owner: RepresentationId,
    floor: u64,
) -> ExchangeResult<State> {
    if let Some(state) = state(connection, owner)? {
        return Ok(state);
    }
    super::super::media_state::require(connection, ObjectRef::Representation(owner), floor)?;
    let current: Option<i64> = connection.query_row("SELECT recorded_revision_sequence FROM dependency_sets WHERE source_representation_id = ?1", [owner.as_bytes().as_slice()], |row| row.get(0))
        .optional().map_err(sqlite_error("read dependency baseline boundary"))?;
    let created: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM checkpoint_media_created WHERE kind = 2 AND id = ?1)",
            [owner.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check authored dependency owner creation"))?;
    let mut baseline = State {
        present: None,
        dirty: None,
        recorded: None,
        count: None,
        baseline: false,
    };
    if floor == 0 || created || current.is_none() {
        baseline.present = Some(false);
    } else if current.is_some_and(|sequence| u64::try_from(sequence).is_ok_and(|s| s <= floor)) {
        let header = dependency_facts::header(connection, owner)?;
        baseline.present = Some(true);
        baseline.recorded = current;
        baseline.count = Some(i64::try_from(header.occurrence_count()).map_err(|_| invalid())?);
        // No retained replacement follows this observation, so its ordered
        // values remain available in the current table. Its earlier dirty
        // status cannot be inferred from the final status.
        baseline.baseline = true;
    }
    store(connection, owner, baseline)?;
    Ok(baseline)
}

pub(super) fn has_observation(
    connection: &Connection,
    owner: RepresentationId,
    sequence: u64,
) -> ExchangeResult<bool> {
    connection.query_row("SELECT EXISTS(SELECT 1 FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind = ?1 AND e.primary_id = ?2 AND r.sequence = ?3)", params![crate::stored_revision_event_kind(postproject_core::RevisionEventType::DependencySetRecorded)?, owner.as_bytes().as_slice(), i64::try_from(sequence).map_err(|_| invalid())?], |row| row.get(0))
        .map_err(sqlite_error("check original dependency observation")).map_err(Into::into)
}
