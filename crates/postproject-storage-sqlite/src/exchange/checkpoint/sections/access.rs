use postproject_core::ResourceId;
use postproject_protocol::{CheckpointChunk, IdentifierAttachment, encode_locator};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

use super::super::{invalid, writer::SectionWriter};

pub(in crate::exchange::checkpoint) fn locators<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let mut statement = view.connection.prepare("SELECT l.id, l.uri, l.last_seen_micros, l.availability, l.media_root_name, n.prefix, n.suffix, n.padding, l.resource_id FROM locators l LEFT JOIN locator_sequence_namings n ON n.locator_id = l.id ORDER BY l.id")
        .map_err(sqlite_error("prepare checkpoint locator facts"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint locator facts"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint locator fact"))?
    {
        let naming = [
            row.get::<_, Option<String>>(5)
                .map_err(sqlite_error("read naming prefix"))?
                .is_some(),
            row.get::<_, Option<String>>(6)
                .map_err(sqlite_error("read naming suffix"))?
                .is_some(),
            row.get::<_, Option<i64>>(7)
                .map_err(sqlite_error("read naming padding"))?
                .is_some(),
        ];
        if naming.iter().any(|present| *present) && !naming.iter().all(|present| *present) {
            return Err(invalid().into());
        }
        let resource = ResourceId::from_bytes(crate::id_bytes(
            row.get(8).map_err(sqlite_error("read locator resource"))?,
            "located resource",
        )?);
        let locator = crate::StoredLocator::read(row)
            .map_err(sqlite_error("read scalar locator"))?
            .into_locator(resource)?;
        writer.document(&encode_locator(&locator)?, true)?;
    }
    Ok(())
}

pub(in crate::exchange::checkpoint) fn identifiers<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    // Native identifier collections retain attachment order within each target.
    // Physical IDs only order the stream; they are never portable facts.
    let mut statement = view.connection.prepare("SELECT target_kind, target_id, scheme, value, qualifier FROM external_identifiers ORDER BY id")
        .map_err(sqlite_error("prepare checkpoint identifier facts"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint identifier facts"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint identifier fact"))?
    {
        let target = crate::decode_identifier_target(
            row.get(0).map_err(sqlite_error("read attachment kind"))?,
            row.get(1)
                .map_err(sqlite_error("read attachment identity"))?,
        )?;
        let identifier = crate::decode_external_identifier(
            row.get(2).map_err(sqlite_error("read identifier scheme"))?,
            row.get(3).map_err(sqlite_error("read identifier value"))?,
            row.get(4)
                .map_err(sqlite_error("read identifier qualifier"))?,
        )?;
        writer.document(
            &IdentifierAttachment::new(target, identifier)?.document()?,
            true,
        )?;
    }
    Ok(())
}
