//! Domain section items are checked before the private transaction can commit.

mod facts;
mod history;
mod records;

use postproject_protocol::{CheckpointManifest, CheckpointSection, Document};
use rusqlite::Transaction;

use crate::ExchangeResult;

pub(super) struct Bodies<'a, 'connection> {
    transaction: &'a Transaction<'connection>,
    manifest: &'a CheckpointManifest,
    revisions: u64,
    event_sequence: u64,
    event_position: u64,
    metadata_key: Option<(i64, Vec<u8>, String, String)>,
    metadata_position: u64,
    record: Option<records::ImportedRecord>,
    record_head: postproject_protocol::Position,
    record_document_limits: postproject_protocol::Limits,
    remaining_frames: u64,
}

impl<'a, 'connection> Bodies<'a, 'connection> {
    pub(super) fn new(
        transaction: &'a Transaction<'connection>,
        manifest: &'a CheckpointManifest,
        limits: super::CheckpointLimits,
    ) -> Self {
        Self {
            transaction,
            manifest,
            revisions: 0,
            event_sequence: 0,
            event_position: 0,
            metadata_key: None,
            metadata_position: 0,
            record: None,
            record_head: manifest.floor(),
            record_document_limits: limits.document,
            remaining_frames: limits.frames,
        }
    }

    pub(super) fn document(
        &mut self,
        section: CheckpointSection,
        document: &Document,
    ) -> ExchangeResult<bool> {
        self.remaining_frames = self
            .remaining_frames
            .checked_sub(1)
            .ok_or_else(super::limits::budget)?;
        match section {
            CheckpointSection::Production => self.production(document)?,
            CheckpointSection::Metadata => self.metadata(document)?,
            CheckpointSection::Revisions => self.revision(document)?,
            CheckpointSection::Events => self.event(document)?,
            CheckpointSection::ConflictVersions => self.version(document)?,
            CheckpointSection::ConflictFloor => self.conflict_floor(document)?,
            CheckpointSection::Records => return self.record(document),
            _ => {
                return Err(postproject_protocol::ProtocolError::new(
                    postproject_protocol::FailureKind::Unsupported,
                    "checkpoint section is not supported yet",
                )
                .into());
            }
        }
        Ok(true)
    }

    pub(super) fn finish(self) -> ExchangeResult<()> {
        if self.revisions != self.manifest.head().sequence()
            || self.event_sequence != self.revisions
            || self.record.is_some()
            || self.record_head != self.manifest.head()
        {
            return Err(super::super::invalid().into());
        }
        self.validate_boundaries()?;
        self.validate_versions()
    }
}
