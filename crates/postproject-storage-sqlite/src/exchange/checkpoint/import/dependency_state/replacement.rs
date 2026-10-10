use postproject_core::{DependencySetStatus, RepresentationId};
use postproject_protocol::{DependencyOccurrence, DependencySetHeader, Document};
use rusqlite::{Connection, params};

use crate::{ExchangeResult, sqlite_error};

use super::{State, baseline, invalid, occurrence, store};

pub(in crate::exchange::checkpoint::import) struct Replacement {
    header: DependencySetHeader,
    next: u64,
    unchanged: bool,
    floor: u64,
}

impl Replacement {
    pub(in crate::exchange::checkpoint::import) fn begin(
        connection: &Connection,
        header: DependencySetHeader,
        sequence: u64,
        floor: u64,
    ) -> ExchangeResult<Self> {
        let owner = header.source_representation_id();
        if header.status() != DependencySetStatus::Current
            || header.recorded_at_revision() != sequence
            || sequence <= floor
            || !baseline::has_observation(connection, owner, sequence)?
        {
            return Err(invalid().into());
        }
        let prior = baseline::ensure(connection, owner, floor)?;
        Ok(Self {
            header,
            next: 0,
            unchanged: prior.present == Some(true)
                && prior.dirty == Some(false)
                && prior.count.and_then(|n| u64::try_from(n).ok())
                    == Some(header.occurrence_count()),
            floor,
        })
    }

    pub(in crate::exchange::checkpoint::import) fn push(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<bool> {
        let value = DependencyOccurrence::from_document(document)?;
        let owner = self.header.source_representation_id();
        if value.source_representation_id() != owner
            || value.position() != self.next
            || self.next >= self.header.occurrence_count()
        {
            return Err(invalid().into());
        }
        occurrence::require_references(connection, &value, self.floor)?;
        if self.unchanged {
            self.unchanged =
                occurrence::prior(connection, owner, self.next)?.as_ref() == Some(&value);
        }
        connection.execute("INSERT INTO checkpoint_dependency_occurrences (owner, position, document) VALUES (?1, ?2, ?3) ON CONFLICT(owner, position) DO UPDATE SET document = excluded.document", params![owner.as_bytes().as_slice(), i64::try_from(self.next).map_err(|_| invalid())?, value.document()?.canonical_bytes()?])
            .map_err(sqlite_error("stage authored dependency occurrence"))?;
        self.next += 1;
        Ok(self.next == self.header.occurrence_count())
    }

    pub(in crate::exchange::checkpoint::import) fn finish(
        self,
        connection: &Connection,
    ) -> ExchangeResult<()> {
        if self.unchanged || self.next != self.header.occurrence_count() {
            return Err(invalid().into());
        }
        let owner: RepresentationId = self.header.source_representation_id();
        connection
            .execute(
                "DELETE FROM checkpoint_dependency_occurrences WHERE owner = ?1 AND position >= ?2",
                params![
                    owner.as_bytes().as_slice(),
                    i64::try_from(self.next).map_err(|_| invalid())?
                ],
            )
            .map_err(sqlite_error(
                "finish authored dependency occurrence replacement",
            ))?;
        store(
            connection,
            owner,
            State {
                present: Some(true),
                dirty: Some(false),
                recorded: Some(
                    i64::try_from(self.header.recorded_at_revision()).map_err(|_| invalid())?,
                ),
                count: Some(i64::try_from(self.next).map_err(|_| invalid())?),
                baseline: false,
            },
        )
    }
}
