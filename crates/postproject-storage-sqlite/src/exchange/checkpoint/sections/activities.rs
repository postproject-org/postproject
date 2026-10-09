use postproject_core::ActivityId;
use postproject_protocol::CheckpointChunk;

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

use super::super::{activity_facts, writer::SectionWriter};

pub(in crate::exchange::checkpoint) fn activities<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let mut statement = view
        .connection
        .prepare("SELECT id FROM activities ORDER BY id")
        .map_err(sqlite_error("prepare checkpoint activity identities"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint activity identities"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint activity identity"))?
    {
        let id = ActivityId::from_bytes(crate::id_bytes(
            row.get(0).map_err(sqlite_error("read activity identity"))?,
            "checkpoint activity",
        )?);
        let header = activity_facts::header(&view.connection, id)?;
        let mut first = true;
        crate::exchange::activity_capture::write(
            &view.connection,
            &header,
            &mut |document| -> ExchangeResult<()> {
                writer.document(document, first)?;
                first = false;
                Ok(())
            },
        )?;
    }
    Ok(())
}
