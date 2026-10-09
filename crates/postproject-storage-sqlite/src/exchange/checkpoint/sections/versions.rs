use postproject_core::{
    DecisionBase, MetadataProperty, PropertyId, RevisionId, SemanticConflictKey, VocabularyId,
};
use postproject_protocol::{CheckpointChunk, ConflictVersion, encode_conflict_floor};
use rusqlite::OptionalExtension;

use crate::{ExchangeResult, SqliteProduction, sqlite_error, transaction::encode_conflict_key};

use super::super::{invalid, writer::SectionWriter};

pub(in crate::exchange::checkpoint) fn conflict_floor<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let baseline = view.connection.query_row("SELECT revision_id, revision_sequence FROM conflict_migration_baseline WHERE singleton = 1", [], |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)))
        .optional().map_err(sqlite_error("read original conflict baseline"))?;
    let (revision, sequence) = if let Some((id, sequence)) = baseline {
        (
            Some(RevisionId::from_bytes(crate::id_bytes(
                id,
                "conflict baseline",
            )?)),
            crate::stored_u64(sequence, "conflict baseline")?,
        )
    } else {
        (None, 0)
    };
    let base = DecisionBase::new(view.production.id(), revision, sequence)?;
    writer.document(&encode_conflict_floor(base), true)
}

pub(in crate::exchange::checkpoint) fn versions<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    // Removed properties still occur in retained original observations. Derive
    // domain keys from that evidence, never put private key bytes on the wire.
    let mut statement = view.connection.prepare("SELECT DISTINCT target_kind, primary_id, vocabulary, property FROM revision_events WHERE kind IN (9, 10) ORDER BY target_kind, primary_id, vocabulary, property")
        .map_err(sqlite_error("prepare metadata semantic versions"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query metadata semantic versions"))?;
    let mut exported = 0_i64;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read semantic version target"))?
    {
        let key = SemanticConflictKey::MetadataProperty {
            target: crate::decode_metadata_target(
                row.get(0)
                    .map_err(sqlite_error("read version target kind"))?,
                row.get(1)
                    .map_err(sqlite_error("read version target identity"))?,
            )?,
            property: MetadataProperty::new(
                VocabularyId::new(
                    row.get::<_, String>(2)
                        .map_err(sqlite_error("read version vocabulary"))?,
                )?,
                PropertyId::new(
                    row.get::<_, String>(3)
                        .map_err(sqlite_error("read version property"))?,
                )?,
            ),
        };
        let version = view.connection.query_row("SELECT last_changed_revision_id, last_changed_revision_sequence FROM conflict_versions WHERE conflict_key = ?1", [encode_conflict_key(&key)?], |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)))
            .optional().map_err(sqlite_error("read original semantic version"))?;
        if let Some((id, sequence)) = version {
            let version = ConflictVersion::new(
                key,
                RevisionId::from_bytes(crate::id_bytes(id, "semantic revision")?),
                crate::stored_u64(sequence, "semantic sequence")?,
            )?;
            writer.document(&version.document()?, true)?;
            exported += 1;
        }
    }
    let stored: i64 = view
        .connection
        .query_row("SELECT count(*) FROM conflict_versions", [], |row| {
            row.get(0)
        })
        .map_err(sqlite_error("verify exported semantic version count"))?;
    if exported != stored {
        return Err(invalid().into());
    }
    Ok(())
}
