use postproject_core::RepresentationId;
use postproject_protocol::{CheckpointChunk, DependencyOccurrence};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

use super::super::{dependency_facts, invalid, writer::SectionWriter};

pub(in crate::exchange::checkpoint) fn dependencies<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
    head: u64,
) -> ExchangeResult<()> {
    let mut statement = view.connection.prepare("SELECT source_representation_id FROM dependency_sets ORDER BY source_representation_id")
        .map_err(sqlite_error("prepare checkpoint dependency identities"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint dependency identities"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint dependency identity"))?
    {
        let owner = RepresentationId::from_bytes(crate::id_bytes(
            row.get(0).map_err(sqlite_error("read dependency owner"))?,
            "dependency owner",
        )?);
        let header = dependency_facts::header(&view.connection, owner)?;
        if header.recorded_at_revision() > head {
            return Err(invalid().into());
        }
        writer.document(&header.document(), true)?;
        let mut occurrences = view.connection.prepare("SELECT position, source_resource_id, kind, target_kind, target_id, resolved_representation_id, required, authored_reference FROM dependencies WHERE source_representation_id = ?1 ORDER BY position")
            .map_err(sqlite_error("prepare checkpoint dependency occurrences"))?;
        let mut values = occurrences
            .query([owner.as_bytes().as_slice()])
            .map_err(sqlite_error("query checkpoint dependency occurrences"))?;
        let mut position = 0_u64;
        while let Some(row) = values
            .next()
            .map_err(sqlite_error("read checkpoint dependency occurrence"))?
        {
            let (
                stored_position,
                resource,
                kind,
                target_kind,
                target,
                resolved,
                required,
                authored,
            ) = crate::read_budget::bounded(|row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            })(row)
            .map_err(sqlite_error("read checkpoint dependency facts"))?;
            if crate::stored_u64(stored_position, "dependency occurrence position")? != position {
                return Err(invalid().into());
            }
            let dependency = crate::decode_dependency(
                resource,
                kind,
                target_kind,
                target,
                resolved,
                required,
                authored,
            )?;
            writer.document(
                &DependencyOccurrence::new(owner, position, dependency)?.document()?,
                false,
            )?;
            position = position.checked_add(1).ok_or_else(invalid)?;
        }
        if position != header.occurrence_count() {
            return Err(invalid().into());
        }
    }
    Ok(())
}
