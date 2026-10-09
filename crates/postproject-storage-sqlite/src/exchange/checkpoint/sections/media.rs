use postproject_core::{AssetId, Representation, RepresentationId, ResourceId};
use postproject_protocol::{CheckpointChunk, CheckpointSection, encode_asset, encode_structure};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

use super::super::{invalid, media_facts, writer::SectionWriter};

pub(in crate::exchange::checkpoint) fn media<Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
    section: CheckpointSection,
) -> ExchangeResult<()> {
    let table = match section {
        CheckpointSection::Assets => "assets",
        CheckpointSection::Resources => "resources",
        CheckpointSection::Representations | CheckpointSection::Structures => "representations",
        _ => return Err(invalid().into()),
    };
    let mut statement = view
        .connection
        .prepare(&format!("SELECT id FROM {table} ORDER BY id"))
        .map_err(sqlite_error("prepare checkpoint media identities"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint media identities"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint media identity"))?
    {
        let id = crate::id_bytes(
            row.get(0).map_err(sqlite_error("read media identity"))?,
            "media identity",
        )?;
        match section {
            CheckpointSection::Assets => writer.document(
                &encode_asset(&media_facts::asset(
                    &view.connection,
                    AssetId::from_bytes(id),
                )?),
                true,
            )?,
            CheckpointSection::Resources => writer.document(
                &media_facts::resource(&view.connection, ResourceId::from_bytes(id))?.document(),
                true,
            )?,
            CheckpointSection::Representations | CheckpointSection::Structures => {
                let id = RepresentationId::from_bytes(id);
                let header = media_facts::representation(&view.connection, id)?;
                if section == CheckpointSection::Representations {
                    writer.document(&header.document()?, true)?;
                } else {
                    let representation = Representation::new(
                        id,
                        header.asset_id(),
                        header.kind(),
                        media_facts::content(&view.connection, id)?,
                        Vec::new(),
                    );
                    for (index, frame) in encode_structure(&representation)?.enumerate() {
                        writer.document(&frame?, index == 0)?;
                    }
                }
            }
            _ => return Err(invalid().into()),
        }
    }
    Ok(())
}
