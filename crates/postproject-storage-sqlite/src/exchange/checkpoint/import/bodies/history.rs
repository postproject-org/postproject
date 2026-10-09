use postproject_core::{ObjectRef, RevisionEvent, RevisionId};
use postproject_protocol::{Document, decode_event, decode_revision_observation};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    ExchangeResult, sqlite_error,
    transaction::{persist_revision_event, persist_revision_header},
};

use super::super::super::invalid;
use super::Bodies;

impl Bodies<'_, '_> {
    pub(super) fn revision(&mut self, document: &Document) -> ExchangeResult<()> {
        let revision = decode_revision_observation(document)?;
        if revision.sequence() != self.revisions + 1 {
            return Err(invalid().into());
        }
        persist_revision_header(self.transaction, &revision)?;
        self.revisions += 1;
        Ok(())
    }

    pub(super) fn event(&mut self, document: &Document) -> ExchangeResult<()> {
        let event = decode_event(document)?;
        let sequence = self.revision_sequence(event.revision_id())?;
        if sequence != self.event_sequence {
            if sequence != self.event_sequence + 1 {
                return Err(invalid().into());
            }
            self.event_sequence = sequence;
            self.event_position = 0;
        }
        if u64::from(event.position()) != self.event_position {
            return Err(invalid().into());
        }
        let target = match event.kind() {
            postproject_core::RevisionEventKind::MetadataAddedOrReplaced { target, .. }
            | postproject_core::RevisionEventKind::MetadataRemoved { target, .. } => *target,
            postproject_core::RevisionEventKind::MediaRootAdded { .. }
            | postproject_core::RevisionEventKind::MediaRootEnabledChanged { .. }
            | postproject_core::RevisionEventKind::MediaRootRemoved { .. } => {
                ObjectRef::Production(self.manifest.head().scope().production())
            }
            _ => return Err(invalid().into()),
        };
        if target != ObjectRef::Production(self.manifest.head().scope().production()) {
            return Err(invalid().into());
        }
        persist_revision_event(
            self.transaction,
            event.revision_id(),
            i64::from(event.position()),
            event.kind(),
        )?;
        super::super::guard_state::observed(self.transaction, &event, sequence)?;
        self.event_position += 1;
        Ok(())
    }

    pub(super) fn revision_sequence(&self, id: RevisionId) -> ExchangeResult<u64> {
        let sequence: Option<i64> = self
            .transaction
            .query_row(
                "SELECT sequence FROM revisions WHERE id = ?1",
                [id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error("validate checkpoint revision identity"))?;
        sequence
            .and_then(|sequence| u64::try_from(sequence).ok())
            .ok_or_else(|| invalid().into())
    }

    pub(super) fn validate_revision(&self, id: RevisionId, sequence: u64) -> ExchangeResult<()> {
        if self.revision_sequence(id)? != sequence {
            return Err(invalid().into());
        }
        Ok(())
    }

    pub(super) fn validate_boundaries(&self) -> ExchangeResult<()> {
        for position in [self.manifest.head(), self.manifest.floor()] {
            if let Some(revision) = position.revision() {
                self.validate_revision(revision, position.sequence())?;
            }
        }
        let mut check = self
            .transaction
            .prepare("PRAGMA foreign_key_check")
            .map_err(sqlite_error("prepare checkpoint reference validation"))?;
        if check
            .query([])
            .map_err(sqlite_error("validate checkpoint references"))?
            .next()
            .map_err(sqlite_error("read invalid checkpoint reference"))?
            .is_some()
        {
            return Err(invalid().into());
        }
        Ok(())
    }
}

pub(super) fn observation(
    connection: &Connection,
    revision: RevisionId,
    position: u32,
) -> ExchangeResult<RevisionEvent> {
    let stored = connection.query_row("SELECT position, kind, target_kind, primary_id, secondary_id, structural_position, vocabulary, property, identifier_scheme, identifier_value, identifier_qualifier, activity_kind, role, fingerprint_algorithm, fingerprint_version FROM revision_events WHERE revision_id = ?1 AND position = ?2", params![revision.as_bytes().as_slice(), i64::from(position)], crate::stored_revision_event_row)
        .optional().map_err(sqlite_error("validate retained observation"))?.ok_or_else(invalid)?;
    Ok(crate::decode_revision_event(revision, stored)?)
}
