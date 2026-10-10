use postproject_core::DecisionBase;
use postproject_protocol::{ArchiveEvidence, ArchiveFamily, Document, Position, ProtocolBase};
use rusqlite::{OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error};

use super::Bodies;
use crate::exchange::checkpoint::invalid;

impl Bodies<'_, '_> {
    pub(super) fn archive(&mut self, document: &Document) -> ExchangeResult<()> {
        let evidence = ArchiveEvidence::from_document(document)?;
        let family = match evidence.family() {
            ArchiveFamily::Anchor => 0_u8,
            ArchiveFamily::Manifest => 1,
            ArchiveFamily::Chunk => 2,
            ArchiveFamily::PartialEffect => 3,
        };
        let coordinate = (
            family,
            evidence.sequence(),
            evidence.position(),
            evidence.fragment(),
        );
        if evidence.sequence() > self.manifest.floor().sequence()
            || self
                .archive_previous
                .is_some_and(|previous| previous >= coordinate)
        {
            return Err(invalid().into());
        }
        self.validate_archive_boundary(&evidence)?;
        let sequence = i64::try_from(evidence.sequence()).map_err(|_| invalid())?;
        let revision = evidence.revision().map(|id| id.as_bytes().to_vec());
        let position = i64::try_from(evidence.position()).map_err(|_| invalid())?;
        let fragment = i64::try_from(evidence.fragment()).map_err(|_| invalid())?;
        match evidence.family() {
            ArchiveFamily::Anchor => {
                self.transaction.execute("INSERT INTO exchange_prior_anchors (floor_sequence, floor_revision_id, anchor_digest) VALUES (?1, ?2, ?3)", params![sequence, revision, evidence.payload()])
                    .map_err(sqlite_error("retain original prior anchor"))?;
            }
            ArchiveFamily::Manifest => {
                self.transaction.execute("INSERT INTO exchange_records (revision_id, sequence, manifest) VALUES (?1, ?2, ?3)", params![revision, sequence, evidence.payload()])
                    .map_err(sqlite_error("retain earlier unqualified manifest"))?;
            }
            ArchiveFamily::Chunk => {
                self.transaction.execute("INSERT INTO exchange_record_chunks (revision_id, position, fragment_position, document) VALUES (?1, ?2, ?3, ?4)", params![revision, position, fragment, evidence.payload()])
                    .map_err(sqlite_error("retain earlier chunk evidence"))?;
            }
            ArchiveFamily::PartialEffect => {
                self.transaction.execute("INSERT INTO exchange_effect_fragments (revision_id, effect_position, fragment_position, payload) VALUES (?1, ?2, ?3, ?4)", params![revision, position, fragment, evidence.payload()])
                    .map_err(sqlite_error("retain earlier partial authored evidence"))?;
            }
        }
        self.archive_previous = Some(coordinate);
        Ok(())
    }

    fn validate_archive_boundary(&self, evidence: &ArchiveEvidence) -> ExchangeResult<()> {
        if let Some(revision) = evidence.revision() {
            let actual: Option<i64> = self
                .transaction
                .query_row(
                    "SELECT sequence FROM revisions WHERE id = ?1",
                    [revision.as_bytes().as_slice()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(sqlite_error("validate earlier evidence revision"))?;
            if actual != Some(i64::try_from(evidence.sequence()).map_err(|_| invalid())?) {
                return Err(invalid().into());
            }
        }
        if evidence.family() == ArchiveFamily::Anchor {
            let base = ProtocolBase::new(
                self.manifest.head().scope(),
                DecisionBase::new(
                    self.manifest.head().scope().production(),
                    evidence.revision(),
                    evidence.sequence(),
                )?,
            )?;
            if evidence.sequence() >= self.manifest.floor().sequence()
                || Position::anchor(base)?.digest().as_bytes() != evidence.payload()
            {
                return Err(invalid().into());
            }
        }
        Ok(())
    }
}
