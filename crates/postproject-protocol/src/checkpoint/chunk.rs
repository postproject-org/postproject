use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;

use crate::{
    CheckpointId, CheckpointSection, Digest, DigestDomain, Document, Extensions,
    MAX_RECORD_CHUNK_PAYLOAD, Result, Scope,
    fields::{limit, malformed},
};

/// Ordered bytes from one checkpoint section, bounded like record chunks.
///
/// Scope, checkpoint identity and section are all integrity-bound. A payload
/// may split a frame or UTF-8 scalar. Neither framing nor a digest authenticates
/// the source or proves that the section contains valid domain facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointChunk {
    scope: Scope,
    checkpoint: CheckpointId,
    section: CheckpointSection,
    index: u64,
    previous: Option<Digest>,
    payload: Vec<u8>,
    extensions: Extensions,
}

impl CheckpointChunk {
    /// Creates a bounded continuation belonging to one explicit section.
    ///
    /// # Errors
    /// Rejects empty/oversized bytes, out-of-range indices and contradictory
    /// predecessor presence. Sections may be empty and then emit no chunks.
    pub fn new(
        scope: Scope,
        checkpoint: CheckpointId,
        section: CheckpointSection,
        index: u64,
        previous: Option<Digest>,
        payload: Vec<u8>,
        extensions: Extensions,
    ) -> Result<Self> {
        if payload.len() > MAX_RECORD_CHUNK_PAYLOAD {
            return Err(limit());
        }
        if payload.is_empty() || i64::try_from(index).is_err() || previous.is_some() != (index != 0)
        {
            return Err(malformed());
        }
        Ok(Self {
            scope,
            checkpoint,
            section,
            index,
            previous,
            payload,
            extensions,
        })
    }

    /// Returns the preserved source production/history scope.
    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }
    /// Returns this export's identity, distinct from source history.
    #[must_use]
    pub const fn checkpoint(&self) -> CheckpointId {
        self.checkpoint
    }
    /// Returns the exact owning section.
    #[must_use]
    pub const fn section(&self) -> CheckpointSection {
        self.section
    }
    /// Returns the zero-based index within this section.
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }
    /// Returns the prior section chunk's digest, absent only at index zero.
    #[must_use]
    pub const fn previous(&self) -> Option<Digest> {
        self.previous
    }
    /// Borrows the exact continuation bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    /// Borrows preserved optional facts bound to these bytes.
    #[must_use]
    pub const fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    fn unsigned_document(&self) -> Document {
        Document {
            value: json!({"kind":"checkpoint.chunk", "version":"1", "required_features":["checkpoints.v1"], "production":self.scope.production().to_string(), "history":self.scope.history().to_string(), "checkpoint":self.checkpoint.to_string(), "section":self.section.as_str(), "index":self.index.to_string(), "previous":self.previous.map(|digest|digest.to_string()), "payload":STANDARD.encode(&self.payload), "extensions":self.extensions.document().value}),
        }
    }

    /// Computes integrity over identity, ordering and exact payload bytes.
    ///
    /// # Errors
    /// Rejects an internally unencodable extension value.
    pub fn digest(&self) -> Result<Digest> {
        self.unsigned_document().digest(DigestDomain::Chunk)
    }

    /// Encodes the full bounded envelope and checked digest.
    ///
    /// # Errors
    /// Rejects an internally unencodable extension value.
    pub fn document(&self) -> Result<Document> {
        let mut document = self.unsigned_document();
        document.value["digest"] = document.digest(DigestDomain::Chunk)?.to_string().into();
        Ok(document)
    }

    /// Decodes exact fields and verifies integrity before returning bytes.
    ///
    /// # Errors
    /// Rejects unknown required kinds/features/sections, invalid base64,
    /// excessive bytes and altered integrity. Domain validation remains required.
    pub fn from_document(document: &Document) -> Result<Self> {
        super::wire::decode_chunk(document)
    }
}
