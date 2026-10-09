mod effects;

use postproject_protocol::{Document, FrameDecoder, RecordChunk, RecordChunkChain, RecordManifest};
use rusqlite::params;

use crate::{ExchangeResult, sqlite_error};

use super::{super::super::invalid, Bodies};

pub(super) struct ImportedRecord {
    manifest: RecordManifest,
    chain: RecordChunkChain,
    decoder: FrameDecoder,
    effects: effects::RetainedEffects,
    chunks: u64,
}

impl Bodies<'_, '_> {
    pub(super) fn record(&mut self, document: &Document) -> ExchangeResult<bool> {
        if self.record.is_some() {
            return self.record_chunk(document);
        }
        let manifest = RecordManifest::from_document(document)?;
        if manifest.predecessor() != self.record_head
            || manifest.head()?.sequence() > self.manifest.head().sequence()
        {
            return Err(invalid().into());
        }
        let stored = self.transaction.query_row("SELECT id, sequence, transaction_id, committed_at_micros, origin_name, origin_version, origin_uri, message FROM revisions WHERE id = ?1", [manifest.revision().id().as_bytes().as_slice()], crate::stored_revision_row)
            .map_err(sqlite_error("validate retained record revision"))?;
        if crate::decode_revision(stored)? != *manifest.revision() {
            return Err(invalid().into());
        }
        let events: i64 = self
            .transaction
            .query_row(
                "SELECT count(*) FROM revision_events WHERE revision_id = ?1",
                [manifest.revision().id().as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(sqlite_error("validate retained record event total"))?;
        if u64::try_from(events).ok() != Some(manifest.event_count())
            || manifest.effect_count() != manifest.event_count()
        {
            return Err(invalid().into());
        }
        let effects =
            effects::RetainedEffects::new(&manifest, self.manifest.floor().sequence() == 0);
        self.record = Some(ImportedRecord {
            chain: manifest.chunk_chain(),
            manifest,
            decoder: FrameDecoder::new(self.record_document_limits),
            effects,
            chunks: 0,
        });
        Ok(true)
    }

    fn record_chunk(&mut self, document: &Document) -> ExchangeResult<bool> {
        let chunk = RecordChunk::from_document(document)?;
        let record = self.record.as_mut().ok_or_else(invalid)?;
        record.chain.push(&chunk)?;
        crate::exchange::records::chunks::persist(self.transaction, &chunk)?;
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, document) = record.decoder.consume(&chunk.payload()[offset..])?;
            offset += consumed;
            if let Some(document) = document {
                self.remaining_frames = self
                    .remaining_frames
                    .checked_sub(1)
                    .ok_or_else(super::super::limits::budget)?;
                record.effects.document(self.transaction, &document)?;
            }
        }
        record.chunks += 1;
        if record.chunks == record.manifest.chunks().count() {
            let mut record = self.record.take().ok_or_else(invalid)?;
            record.manifest.verify_chain(record.chain)?;
            record.decoder.finish()?;
            record.effects.finish()?;
            self.transaction.execute("INSERT INTO exchange_records (revision_id, sequence, manifest) VALUES (?1, ?2, ?3)", params![record.manifest.revision().id().as_bytes().as_slice(), i64::try_from(record.manifest.revision().sequence()).map_err(|_| invalid())?, record.manifest.document()?.canonical_bytes()?])
                .map_err(sqlite_error("stage retained original record"))?;
            self.record_head = record.manifest.head()?;
        }
        Ok(false)
    }
}
