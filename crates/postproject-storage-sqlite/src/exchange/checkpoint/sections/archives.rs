use postproject_core::{DecisionBase, RevisionId};
use postproject_protocol::{
    ArchiveEvidence, ArchiveFamily, CheckpointChunk, Position, ProtocolBase,
};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

use super::super::{invalid, writer::SectionWriter};

pub(in crate::exchange::checkpoint) fn archives<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    floor: Position,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let contradictory: bool = view.connection.query_row("SELECT EXISTS(SELECT 1 FROM exchange_prior_anchors WHERE floor_sequence >= ?1) OR EXISTS(SELECT 1 FROM exchange_records e JOIN revisions r ON r.id = e.revision_id WHERE e.sequence != r.sequence)", [i64::try_from(floor.sequence()).map_err(|_| invalid())?], |row| row.get(0))
        .map_err(sqlite_error("validate original archive boundaries"))?;
    if contradictory {
        return Err(invalid().into());
    }
    let mut statement = view.connection.prepare("SELECT family, revision, sequence, position, fragment, payload FROM (
        SELECT 0 AS family, floor_revision_id AS revision, floor_sequence AS sequence, 0 AS position, 0 AS fragment, anchor_digest AS payload FROM exchange_prior_anchors
        UNION ALL SELECT 1, e.revision_id, r.sequence, 0, 0, e.manifest FROM exchange_records e JOIN revisions r ON r.id = e.revision_id
        UNION ALL SELECT 2, e.revision_id, r.sequence, e.position, e.fragment_position, e.document FROM exchange_record_chunks e JOIN revisions r ON r.id = e.revision_id
        UNION ALL SELECT 3, e.revision_id, r.sequence, e.effect_position, e.fragment_position, e.payload FROM exchange_effect_fragments e JOIN revisions r ON r.id = e.revision_id
        ) WHERE sequence <= ?1 ORDER BY family, sequence, position, fragment")
        .map_err(sqlite_error("prepare earlier history archive"))?;
    let mut rows = statement
        .query([i64::try_from(floor.sequence()).map_err(|_| invalid())?])
        .map_err(sqlite_error("query earlier history archive"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read earlier history evidence"))?
    {
        let family = match row
            .get::<_, i64>(0)
            .map_err(sqlite_error("read archive family"))?
        {
            0 => ArchiveFamily::Anchor,
            1 => ArchiveFamily::Manifest,
            2 => ArchiveFamily::Chunk,
            3 => ArchiveFamily::PartialEffect,
            _ => return Err(invalid().into()),
        };
        let revision = row
            .get::<_, Option<Vec<u8>>>(1)
            .map_err(sqlite_error("read archive revision"))?
            .map(|bytes| crate::id_bytes(bytes, "archive revision").map(RevisionId::from_bytes))
            .transpose()?;
        let sequence = crate::stored_u64(
            row.get(2).map_err(sqlite_error("read archive boundary"))?,
            "archive sequence",
        )?;
        if family == ArchiveFamily::Anchor && sequence >= floor.sequence() {
            return Err(invalid().into());
        }
        let evidence = ArchiveEvidence::new(
            family,
            revision,
            sequence,
            crate::stored_u64(
                row.get(3)
                    .map_err(sqlite_error("read archive item position"))?,
                "archive position",
            )?,
            crate::stored_u64(
                row.get(4)
                    .map_err(sqlite_error("read archive byte position"))?,
                "archive fragment",
            )?,
            row.get(5)
                .map_err(sqlite_error("read exact archive bytes"))?,
        )?;
        if family == ArchiveFamily::Anchor {
            let base = ProtocolBase::new(
                floor.scope(),
                DecisionBase::new(floor.scope().production(), revision, sequence)?,
            )?;
            if Position::anchor(base)?.digest().as_bytes() != evidence.payload() {
                return Err(invalid().into());
            }
        }
        writer.document(&evidence.document(), true)?;
    }
    Ok(())
}
