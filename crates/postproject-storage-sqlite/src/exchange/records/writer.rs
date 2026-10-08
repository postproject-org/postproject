use postproject_core::{Result, RevisionId};
use postproject_protocol::{
    ChunkSummary, Document, Extensions, RecordChunk, RecordChunkChain, Scope,
};
use rusqlite::{Connection, params};

use crate::sqlite_error;

// Keep stored envelopes below SQLite's 16 MiB untrusted-value bound.
const PAYLOAD_BYTES: usize = 1024 * 1024;

pub(super) struct RecordWriter<'a> {
    connection: &'a Connection,
    scope: Scope,
    revision: RevisionId,
    chain: RecordChunkChain,
    previous: Option<postproject_protocol::Digest>,
    index: u64,
    buffer: Vec<u8>,
}

impl<'a> RecordWriter<'a> {
    pub(super) fn new(connection: &'a Connection, scope: Scope, revision: RevisionId) -> Self {
        Self {
            connection,
            scope,
            revision,
            chain: RecordChunkChain::new(scope, revision),
            previous: None,
            index: 0,
            buffer: Vec::with_capacity(PAYLOAD_BYTES),
        }
    }

    pub(super) fn document(&mut self, document: &Document) -> Result<()> {
        let bytes = document.canonical_bytes().map_err(|_| super::encoding())?;
        let length = u64::try_from(bytes.len()).map_err(|_| super::encoding())?;
        self.bytes(&length.to_be_bytes())?;
        self.bytes(&bytes)
    }

    fn bytes(&mut self, mut bytes: &[u8]) -> Result<()> {
        while !bytes.is_empty() {
            let count = bytes.len().min(PAYLOAD_BYTES - self.buffer.len());
            self.buffer.extend_from_slice(&bytes[..count]);
            bytes = &bytes[count..];
            if self.buffer.len() == PAYLOAD_BYTES {
                self.flush()?;
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let chunk = RecordChunk::new(
            self.scope,
            self.revision,
            self.index,
            self.previous,
            std::mem::replace(&mut self.buffer, Vec::with_capacity(PAYLOAD_BYTES)),
            Extensions::default(),
        )
        .map_err(|_| super::encoding())?;
        self.chain.push(&chunk).map_err(|_| super::encoding())?;
        self.previous = Some(chunk.digest().map_err(|_| super::encoding())?);
        let document = chunk
            .document()
            .and_then(|document| document.canonical_bytes())
            .map_err(|_| super::encoding())?;
        self.connection.execute("INSERT INTO exchange_record_chunks (revision_id, position, document) VALUES (?1, ?2, ?3)", params![self.revision.as_bytes().as_slice(), i64::try_from(self.index).map_err(|_| super::encoding())?, document])
            .map_err(sqlite_error("persist committed record chunk"))?;
        self.index = self.index.checked_add(1).ok_or_else(super::encoding)?;
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<ChunkSummary> {
        self.flush()?;
        self.chain.finish().map_err(|_| super::encoding())
    }
}
