use postproject_core::JobId;
use postproject_protocol::CheckpointChunk;

use crate::{ExchangeResult, SqliteProduction, exchange::job_capture, sqlite_error};

use super::super::writer::SectionWriter;
#[cfg(test)]
mod tests;

pub(in crate::exchange::checkpoint) fn jobs<Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let mut statement = view
        .connection
        .prepare("SELECT id FROM jobs ORDER BY id")
        .map_err(sqlite_error("prepare checkpoint job identities"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint job identities"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint job identity"))?
    {
        let id = JobId::from_bytes(crate::id_bytes(
            row.get(0).map_err(sqlite_error("read job identity"))?,
            "job identity",
        )?);
        let header = job_capture::header(&view.connection, id)?;
        job_capture::write(&view.connection, &header, |document| {
            writer.document(document, document.kind()? == "job.header")
        })?;
    }
    Ok(())
}
