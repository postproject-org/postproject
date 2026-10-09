use postproject_core::RevisionId;
use postproject_protocol::{CheckpointChunk, Position, encode_event, encode_revision_observation};
use rusqlite::params;

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

use super::super::{invalid, writer::SectionWriter};

pub(in crate::exchange::checkpoint) fn revisions<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let mut statement = view.connection.prepare("SELECT id, sequence, transaction_id, committed_at_micros, origin_name, origin_version, origin_uri, message FROM revisions ORDER BY sequence")
        .map_err(sqlite_error("prepare checkpoint revisions"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint revisions"))?;
    let mut sequence = 0;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint revision"))?
    {
        let revision = crate::decode_revision(
            crate::stored_revision_row(row).map_err(sqlite_error("read revision facts"))?,
        )?;
        sequence += 1;
        if revision.sequence() != sequence {
            return Err(invalid().into());
        }
        writer.document(&encode_revision_observation(&revision)?, true)?;
    }
    Ok(())
}

pub(in crate::exchange::checkpoint) fn events<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let mut statement = view.connection.prepare("SELECT e.position, e.kind, e.target_kind, e.primary_id, e.secondary_id, e.structural_position, e.vocabulary, e.property, e.identifier_scheme, e.identifier_value, e.identifier_qualifier, e.activity_kind, e.role, e.fingerprint_algorithm, e.fingerprint_version, e.revision_id, r.sequence FROM revision_events e JOIN revisions r ON r.id = e.revision_id ORDER BY r.sequence, e.position")
        .map_err(sqlite_error("prepare checkpoint observations"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint observations"))?;
    let mut previous = None;
    let mut position = 0;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint observation"))?
    {
        let revision = RevisionId::from_bytes(crate::id_bytes(
            row.get(15)
                .map_err(sqlite_error("read observation revision"))?,
            "observation revision",
        )?);
        if previous != Some(revision) {
            position = 0;
            previous = Some(revision);
        }
        let event = crate::decode_revision_event(
            revision,
            crate::stored_revision_event_row(row)
                .map_err(sqlite_error("read observation facts"))?,
        )?;
        if u64::from(event.position()) != position {
            return Err(invalid().into());
        }
        writer.document(&encode_event(&event)?, true)?;
        position += 1;
    }
    Ok(())
}

pub(in crate::exchange::checkpoint) fn records<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    floor: Position,
    head: Position,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    let mut predecessor = floor;
    for sequence in floor.sequence() + 1..=head.sequence() {
        let manifest =
            crate::exchange::records::manifest(&view.connection, view.production.id(), sequence)?
                .ok_or_else(invalid)?;
        let revision = view.changes_since(sequence - 1, 1)?;
        if manifest.predecessor() != predecessor || revision.first() != Some(manifest.revision()) {
            return Err(invalid().into());
        }
        writer.document(&manifest.document()?, true)?;
        let mut chain = manifest.chunk_chain();
        for index in 0..manifest.chunks().count() {
            let chunk = crate::exchange::records::chunks::load(
                &view.connection,
                manifest.revision().id(),
                index,
            )?
            .ok_or_else(invalid)?;
            chain.push(&chunk)?;
            writer.document(&chunk.document()?, false)?;
        }
        let count: i64 = view.connection.query_row("SELECT count(DISTINCT position) FROM exchange_record_chunks WHERE revision_id = ?1", params![manifest.revision().id().as_bytes().as_slice()], |row| row.get(0))
            .map_err(sqlite_error("verify checkpoint retained chunks"))?;
        if u64::try_from(count).ok() != Some(manifest.chunks().count()) {
            return Err(invalid().into());
        }
        manifest.verify_chain(chain)?;
        predecessor = manifest.head()?;
    }
    if predecessor != head {
        return Err(invalid().into());
    }
    Ok(())
}
