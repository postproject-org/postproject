//! Stream source rows through checked domain codecs, never a whole section Vec.

mod access;
mod activities;
mod dependencies;
mod fingerprints;
mod history;
pub(in crate::exchange::checkpoint) mod jobs;
mod media;
mod roots;
mod versions;

pub(super) use access::{identifiers, locators};
pub(super) use activities::activities;
pub(super) use dependencies::dependencies;
pub(super) use fingerprints::fingerprints;
pub(super) use history::{events, records, revisions};
pub(super) use jobs::jobs;
pub(super) use media::media;
pub(super) use roots::roots;
pub(super) use versions::{conflict_floor, versions};

use postproject_core::{MetadataProperty, PropertyId, VocabularyId};
use postproject_protocol::{CheckpointChunk, SnapshotAssertion};

use crate::{ExchangeResult, SqliteProduction, metadata_codec, sqlite_error};

use super::writer::SectionWriter;

pub(super) fn metadata<Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let mut statement = view.connection.prepare("SELECT target_kind, target_id, vocabulary, property, position, encoded_value FROM metadata_assertions ORDER BY target_kind, target_id, vocabulary, property, position")
        .map_err(sqlite_error("prepare checkpoint metadata"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint metadata"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint metadata"))?
    {
        let target = crate::decode_metadata_target(
            row.get(0)
                .map_err(sqlite_error("read metadata target kind"))?,
            row.get(1)
                .map_err(sqlite_error("read metadata target identity"))?,
        )?;
        let property = MetadataProperty::new(
            VocabularyId::new(
                row.get::<_, String>(2)
                    .map_err(sqlite_error("read metadata vocabulary"))?,
            )?,
            PropertyId::new(
                row.get::<_, String>(3)
                    .map_err(sqlite_error("read metadata property"))?,
            )?,
        );
        let position = crate::stored_u64(
            row.get(4).map_err(sqlite_error("read metadata position"))?,
            "metadata position",
        )?;
        let bytes: Vec<u8> = row.get(5).map_err(sqlite_error("read metadata value"))?;
        let assertion =
            SnapshotAssertion::new(target, property, position, metadata_codec::decode(&bytes)?)?;
        writer.document(&assertion.document()?, true)?;
    }
    Ok(())
}
