use postproject_core::{JobState, ObjectRef, RevisionEventType};
use postproject_protocol::{JobHeader, JobInput};
use rusqlite::{Connection, params};

use crate::{ExchangeResult, exchange::job_capture, sqlite_error};

use super::{expected, invalid, store, with_state};

pub(in crate::exchange::checkpoint::import) struct Request {
    header: JobHeader,
    next: u64,
    floor: u64,
}

impl Request {
    pub(in crate::exchange::checkpoint::import) fn begin(
        connection: &Connection,
        header: JobHeader,
        floor: u64,
    ) -> ExchangeResult<Self> {
        let current = job_capture::header(connection, header.id())?;
        if header != with_state(&current, JobState::Requested)?
            || expected(connection, header.id())?.is_some()
        {
            return Err(invalid().into());
        }
        super::super::media_state::require(
            connection,
            ObjectRef::Asset(header.requested_output().asset_id()),
            floor,
        )?;
        if let Some(root) = header.requested_output().target_root() {
            super::super::root_state::require_name(connection, root, floor == 0)?;
        }
        store(connection, &header, true, None)?;
        Ok(Self {
            header,
            next: 0,
            floor,
        })
    }

    pub(in crate::exchange::checkpoint::import) fn input(
        &mut self,
        connection: &Connection,
        input: JobInput,
    ) -> ExchangeResult<bool> {
        if input.job_id() != self.header.id()
            || input.position() != self.next
            || self.next >= self.header.input_count()
        {
            return Err(invalid().into());
        }
        let same: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM job_inputs WHERE job_id = ?1 AND position = ?2 AND representation_id = ?3)", params![input.job_id().as_bytes().as_slice(), i64::try_from(input.position()).map_err(|_| invalid())?, input.representation_id().as_bytes().as_slice()], |row| row.get(0))
            .map_err(sqlite_error("validate original immutable job input"))?;
        if !same {
            return Err(invalid().into());
        }
        super::super::media_state::require(
            connection,
            ObjectRef::Representation(input.representation_id()),
            self.floor,
        )?;
        self.next += 1;
        Ok(self.next == self.header.input_count())
    }
}

pub(in crate::exchange::checkpoint::import) fn require(
    connection: &Connection,
    target: ObjectRef,
    floor: u64,
) -> ExchangeResult<()> {
    let ObjectRef::Job(job) = target else {
        return Ok(());
    };
    let allowed: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM jobs j WHERE j.id = ?1 AND (EXISTS(SELECT 1 FROM checkpoint_job_state s WHERE s.job = j.id AND s.created = 1) OR (?2 > 0 AND NOT EXISTS(SELECT 1 FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.primary_id = j.id AND e.kind = ?3 AND r.sequence > ?2))))", params![job.as_bytes().as_slice(), i64::try_from(floor).map_err(|_| invalid())?, crate::stored_revision_event_kind(RevisionEventType::JobRequested)?], |row| row.get(0))
        .map_err(sqlite_error("check job creation before retained reference"))?;
    if !allowed {
        return Err(invalid().into());
    }
    Ok(())
}
