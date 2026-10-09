use postproject_core::{
    MetadataProperty, ObjectRef, PropertyId, RevisionEvent, RevisionId, SemanticConflictKey,
    VocabularyId,
};
use postproject_protocol::{Document, decode_event, decode_revision_observation};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    ExchangeResult, sqlite_error,
    transaction::{encode_conflict_key, persist_revision_event, persist_revision_header},
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

    pub(super) fn validate_versions(&self) -> ExchangeResult<()> {
        let baseline: i64 = self.transaction.query_row("SELECT coalesce((SELECT revision_sequence FROM conflict_migration_baseline WHERE singleton = 1), 0)", [], |row| row.get(0))
            .map_err(sqlite_error("read imported conflict baseline"))?;
        let mut statement = self.transaction.prepare("SELECT e.target_kind, e.primary_id, e.vocabulary, e.property, max(r.sequence) FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind IN (9, 10) GROUP BY e.target_kind, e.primary_id, e.vocabulary, e.property HAVING max(r.sequence) > ?1")
            .map_err(sqlite_error("prepare imported semantic guard validation"))?;
        let mut rows = statement
            .query([baseline])
            .map_err(sqlite_error("query imported semantic guards"))?;
        let mut matched = 0_i64;
        while let Some(row) = rows
            .next()
            .map_err(sqlite_error("read imported semantic guard"))?
        {
            let key = SemanticConflictKey::MetadataProperty {
                target: crate::decode_metadata_target(
                    row.get(0).map_err(sqlite_error("read guard kind"))?,
                    row.get(1).map_err(sqlite_error("read guard target"))?,
                )?,
                property: MetadataProperty::new(
                    VocabularyId::new(
                        row.get::<_, String>(2)
                            .map_err(sqlite_error("read guard vocabulary"))?,
                    )?,
                    PropertyId::new(
                        row.get::<_, String>(3)
                            .map_err(sqlite_error("read guard property"))?,
                    )?,
                ),
            };
            let expected: i64 = row.get(4).map_err(sqlite_error("read guard sequence"))?;
            let actual: Option<i64> = self.transaction.query_row("SELECT last_changed_revision_sequence FROM conflict_versions WHERE conflict_key = ?1", [encode_conflict_key(&key)?], |row| row.get(0))
                .optional().map_err(sqlite_error("validate imported semantic version"))?;
            if actual != Some(expected) {
                return Err(invalid().into());
            }
            matched += 1;
        }
        let mut statement = self.transaction.prepare("SELECT e.primary_id, max(r.sequence) FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind IN (6, 15, 16) GROUP BY e.primary_id HAVING max(r.sequence) > ?1")
            .map_err(sqlite_error("prepare imported root guard validation"))?;
        let mut rows = statement
            .query([baseline])
            .map_err(sqlite_error("query imported root guards"))?;
        while let Some(row) = rows
            .next()
            .map_err(sqlite_error("read imported root guard"))?
        {
            let key = SemanticConflictKey::MediaRoot(postproject_core::MediaRootId::from_bytes(
                crate::id_bytes(
                    row.get(0)
                        .map_err(sqlite_error("read root guard identity"))?,
                    "root guard",
                )?,
            ));
            let expected: i64 = row
                .get(1)
                .map_err(sqlite_error("read root guard boundary"))?;
            let actual: Option<i64> = self.transaction.query_row("SELECT last_changed_revision_sequence FROM conflict_versions WHERE conflict_key = ?1", [encode_conflict_key(&key)?], |row| row.get(0))
                .optional().map_err(sqlite_error("validate imported root guard"))?;
            if actual != Some(expected) {
                return Err(invalid().into());
            }
            matched += 1;
        }
        let total: i64 = self
            .transaction
            .query_row("SELECT count(*) FROM conflict_versions", [], |row| {
                row.get(0)
            })
            .map_err(sqlite_error("count imported semantic versions"))?;
        if matched != total {
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
