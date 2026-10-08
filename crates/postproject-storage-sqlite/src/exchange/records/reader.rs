use postproject_core::{Error, ErrorKind};
use postproject_protocol::{
    Document, FailureKind, Limits, ProtocolError, RecordChunk, RecordChunkChain, RecordManifest,
};
use rusqlite::{OptionalExtension, params};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

/// Streams one committed record through an independently pinned read view.
///
/// Each call owns at most one chunk. Dropping the reader cancels export and
/// releases its view. Completion and failure also release the pinned connection.
pub struct RecordReader {
    view: Option<SqliteProduction>,
    manifest: RecordManifest,
    chain: Option<RecordChunkChain>,
    next: u64,
    complete: bool,
}

impl RecordReader {
    pub(crate) fn open(view: SqliteProduction, sequence: u64) -> ExchangeResult<Self> {
        if i64::try_from(sequence).is_err() {
            return Err(ProtocolError::new(
                FailureKind::Malformed,
                "record sequence is out of range",
            )
            .into());
        }
        let manifest = super::manifest(&view.connection, view.production.id(), sequence)?
            .ok_or_else(|| {
                ProtocolError::new(
                    FailureKind::HistoryGap,
                    "complete record is unavailable at this sequence",
                )
            })?;
        let previous = super::position(&view.connection, view.production.id(), sequence - 1)?;
        if previous != Some(manifest.predecessor()) {
            return Err(super::invalid().into());
        }
        if view.changes_since(sequence - 1, 1)?.first() != Some(manifest.revision()) {
            return Err(super::invalid().into());
        }
        let chain = Some(manifest.chunk_chain());
        Ok(Self {
            view: Some(view),
            manifest,
            chain,
            next: 0,
            complete: false,
        })
    }

    /// Returns this record's bounded original revision and body commitment.
    #[must_use]
    pub const fn manifest(&self) -> &RecordManifest {
        &self.manifest
    }

    /// Returns whether every advertised chunk has passed integrity verification.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    /// Reads and verifies the next bounded chunk; complete readers return `None`.
    ///
    /// Success on the last chunk verifies the full manifest and releases the
    /// view. Earlier chunks alone never constitute a complete exported record.
    ///
    /// # Errors
    /// Returns storage failures or typed integrity/gap failures. Any failure is
    /// terminal; further reads fail rather than resembling successful completion.
    pub fn next_chunk(&mut self) -> ExchangeResult<Option<RecordChunk>> {
        if self.complete {
            return Ok(None);
        }
        if self.view.is_none() {
            return Err(
                Error::new(ErrorKind::Conflict, "record reader is closed after failure").into(),
            );
        }
        let result = self.read_next();
        if result.is_err() {
            self.view = None;
            self.chain = None;
        }
        result
    }

    fn read_next(&mut self) -> ExchangeResult<Option<RecordChunk>> {
        let view = self.view.as_ref().ok_or_else(super::invalid)?;
        let revision = self.manifest.revision().id();
        let bytes = view.connection.query_row("SELECT document FROM exchange_record_chunks WHERE revision_id = ?1 AND position = ?2", params![revision.as_bytes().as_slice(), i64::try_from(self.next).map_err(|_| super::invalid())?], |row| row.get::<_, Vec<u8>>(0))
            .optional().map_err(sqlite_error("read committed record chunk"))?
            .ok_or_else(|| ProtocolError::new(FailureKind::HistoryGap, "committed record chunk is missing"))?;
        let chunk = RecordChunk::from_document(&Document::parse(
            &bytes,
            Limits::new(2_097_152, 192, 8192)?,
        )?)?;
        self.chain
            .as_mut()
            .ok_or_else(super::invalid)?
            .push(&chunk)?;
        self.next = self.next.checked_add(1).ok_or_else(super::invalid)?;
        if self.next == self.manifest.chunks().count() {
            let count: i64 = view
                .connection
                .query_row(
                    "SELECT count(*) FROM exchange_record_chunks WHERE revision_id = ?1",
                    [revision.as_bytes().as_slice()],
                    |row| row.get(0),
                )
                .map_err(sqlite_error("verify committed record chunk count"))?;
            if u64::try_from(count).ok() != Some(self.next) {
                return Err(ProtocolError::new(
                    FailureKind::Integrity,
                    "record has unadvertised chunks",
                )
                .into());
            }
            self.manifest
                .verify_chain(self.chain.take().ok_or_else(super::invalid)?)?;
            self.complete = true;
            self.view = None;
        }
        Ok(Some(chunk))
    }
}
