use postproject_core::{DecisionBase, RevisionId};
use postproject_protocol::{CheckpointChunk, ConflictVersion, encode_conflict_floor};
use rusqlite::OptionalExtension;

use crate::{ExchangeResult, SqliteProduction, sqlite_error, transaction::decode_conflict_key};

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
    // Decode every actual index key into checked domain facts. Removed facts
    // remain represented; no private key bytes or physical row IDs are exported.
    let mut statement = view.connection.prepare("SELECT c.conflict_key, c.last_changed_revision_id, c.last_changed_revision_sequence, r.sequence FROM conflict_versions c LEFT JOIN revisions r ON r.id = c.last_changed_revision_id ORDER BY c.conflict_key")
        .map_err(sqlite_error("prepare original semantic versions"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query original semantic versions"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read original semantic version"))?
    {
        let encoded: Vec<u8> = row
            .get(0)
            .map_err(sqlite_error("read private semantic key"))?;
        let key = decode_conflict_key(&encoded)?;
        let sequence: i64 = row.get(2).map_err(sqlite_error("read semantic boundary"))?;
        let actual: Option<i64> = row
            .get(3)
            .map_err(sqlite_error("read semantic revision boundary"))?;
        if actual != Some(sequence) {
            return Err(invalid().into());
        }
        let version = ConflictVersion::new(
            key,
            RevisionId::from_bytes(crate::id_bytes(
                row.get(1).map_err(sqlite_error("read semantic revision"))?,
                "semantic revision",
            )?),
            crate::stored_u64(sequence, "semantic sequence")?,
        )?;
        writer.document(&version.document()?, true)?;
    }
    Ok(())
}
