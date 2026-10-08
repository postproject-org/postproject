//! Bounded byte continuations for one authoritative logical record.

mod chain;
mod wire;

pub use chain::{ChunkSummary, RecordChunkChain};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use postproject_core::RevisionId;
use serde_json::json;

use crate::{
    Digest, DigestDomain, Document, Extensions, Result, Scope,
    fields::{limit, malformed},
};

/// Maximum encoded stream-chunk size, including its integrity envelope.
pub const MAX_RECORD_CHUNK_BYTES: usize = 32 * 1024 * 1024;
/// Maximum raw continuation bytes; leaves room for base64 and bounded headers.
pub const MAX_RECORD_CHUNK_PAYLOAD: usize = 16 * 1024 * 1024;

/// One ordered, integrity-bound continuation of a committed record body.
///
/// Payload may split a UTF-8 scalar or a large aggregate. Consumers reconstruct
/// and validate complete domain items before making any revision visible.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordChunk {
    scope: Scope,
    revision: RevisionId,
    index: u64,
    previous: Option<Digest>,
    payload: Vec<u8>,
    extensions: Extensions,
}

impl RecordChunk {
    /// Constructs a bounded continuation, without authorizing its contents.
    ///
    /// # Errors
    /// Rejects empty/oversized payloads, out-of-range indices, or a predecessor
    /// present at index zero or absent at a later index.
    pub fn new(
        scope: Scope,
        revision: RevisionId,
        index: u64,
        previous: Option<Digest>,
        payload: Vec<u8>,
        extensions: Extensions,
    ) -> Result<Self> {
        if payload.len() > MAX_RECORD_CHUNK_PAYLOAD {
            return Err(limit());
        }
        if payload.is_empty() || index > i64::MAX as u64 || previous.is_some() != (index != 0) {
            return Err(malformed());
        }
        Ok(Self {
            scope,
            revision,
            index,
            previous,
            payload,
            extensions,
        })
    }

    /// Returns the original authority scope.
    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }
    /// Returns the original revision owning every continuation.
    #[must_use]
    pub const fn revision(&self) -> RevisionId {
        self.revision
    }
    /// Returns the zero-based index within that revision.
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }
    /// Returns the immediately preceding chunk's digest, absent only at zero.
    #[must_use]
    pub const fn previous(&self) -> Option<Digest> {
        self.previous
    }
    /// Borrows exact bytes; individual continuations need not be valid JSON.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    /// Borrows preserved noncritical extension facts.
    #[must_use]
    pub const fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    fn unsigned_document(&self) -> Document {
        Document {
            value: json!({
                "kind":"record.chunk","version":"1","required_features":["record-chunks.v1"],
                "production":self.scope.production().to_string(),"history":self.scope.history().to_string(),
                "revision":self.revision.to_string(),"index":self.index.to_string(),
                "previous":self.previous.map(|digest| digest.to_string()),
                "payload":STANDARD.encode(&self.payload),"extensions":self.extensions.document().value
            }),
        }
    }

    /// Computes integrity over scope, order, predecessor, bytes and extensions.
    ///
    /// # Errors
    /// Rejects an internally unencodable extension value.
    pub fn digest(&self) -> Result<Digest> {
        self.unsigned_document().digest(DigestDomain::Chunk)
    }

    /// Encodes a complete bounded envelope, including its own checked digest.
    ///
    /// # Errors
    /// Rejects an internally unencodable extension value.
    pub fn document(&self) -> Result<Document> {
        let mut document = self.unsigned_document();
        document.value["digest"] = document.digest(DigestDomain::Chunk)?.to_string().into();
        Ok(document)
    }

    /// Decodes a strict envelope and verifies its digest before returning bytes.
    ///
    /// # Errors
    /// Rejects unsupported required features, malformed framing/base64, exceeded
    /// limits and altered contents. Replay still validates the complete chain
    /// and reconstructed domain items before applying anything.
    pub fn from_document(document: &Document) -> Result<Self> {
        wire::decode(document)
    }
}
