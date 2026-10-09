use postproject_core::{MediaRoot, MediaRootId};
use postproject_protocol::{CheckpointChunk, encode_root};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

use super::super::writer::SectionWriter;

pub(in crate::exchange::checkpoint) fn roots<Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let mut statement = view
        .connection
        .prepare(
            "SELECT id, name, label, legacy_uri, priority, enabled FROM media_roots ORDER BY id",
        )
        .map_err(sqlite_error("prepare checkpoint roots"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint roots"))?;
    while let Some(row) = rows.next().map_err(sqlite_error("read checkpoint root"))? {
        let root = MediaRoot::new(
            MediaRootId::from_bytes(crate::id_bytes(
                row.get(0).map_err(sqlite_error("read root identity"))?,
                "root",
            )?),
            row.get::<_, String>(1)
                .map_err(sqlite_error("read root name"))?,
            row.get(2).map_err(sqlite_error("read root label"))?,
            row.get(3).map_err(sqlite_error("read root fallback"))?,
            row.get(4).map_err(sqlite_error("read root priority"))?,
            row.get(5).map_err(sqlite_error("read root state"))?,
        )?;
        writer.document(&encode_root(&root), true)?;
    }
    Ok(())
}
