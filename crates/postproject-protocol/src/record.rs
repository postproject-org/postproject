//! Bounded manifests binding original revisions to complete chunk chains.

use postproject_core::Revision;
use serde_json::json;

use crate::{
    ChunkSummary, Digest, DigestDomain, Document, Extensions, FailureKind, Position, ProtocolError,
    RecordChunkChain, Result, fields::malformed, receipt::encode_revision,
};

/// One committed revision's bounded header and complete body commitment.
///
/// Chunk payloads contain domain effects and observation events. This header
/// binds their ordered bytes without collecting all chunk descriptors in memory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordManifest {
    predecessor: Position,
    revision: Revision,
    chunks: ChunkSummary,
    effects: u64,
    events: u64,
    extensions: Extensions,
}

impl RecordManifest {
    /// Constructs the next record in an explicitly scoped source history.
    ///
    /// # Errors
    /// Rejects noncontiguous or repeated revisions, empty records and counts
    /// outside the supported persisted integer range.
    pub fn new(
        predecessor: Position,
        revision: Revision,
        chunks: ChunkSummary,
        effects: u64,
        events: u64,
        extensions: Extensions,
    ) -> Result<Self> {
        if predecessor.sequence().checked_add(1) != Some(revision.sequence())
            || predecessor.revision() == Some(revision.id())
            || ![revision.sequence(), effects, events]
                .into_iter()
                .all(|count| count > 0 && i64::try_from(count).is_ok())
        {
            return Err(malformed());
        }
        Ok(Self {
            predecessor,
            revision,
            chunks,
            effects,
            events,
            extensions,
        })
    }

    /// Returns the immediate complete source boundary before this revision.
    #[must_use]
    pub const fn predecessor(&self) -> Position {
        self.predecessor
    }
    /// Returns the authority's original revision, transaction, time and context.
    #[must_use]
    pub const fn revision(&self) -> &Revision {
        &self.revision
    }
    /// Returns the complete advertised chunk chain commitment.
    #[must_use]
    pub const fn chunks(&self) -> ChunkSummary {
        self.chunks
    }
    /// Returns the number of authored effects, including historical changes.
    #[must_use]
    pub const fn effect_count(&self) -> u64 {
        self.effects
    }
    /// Returns the number of public observation events.
    #[must_use]
    pub const fn event_count(&self) -> u64 {
        self.events
    }
    /// Returns preserved, uninterpreted optional extension values.
    #[must_use]
    pub const fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Starts a constant-space verifier for this record's exact source identity.
    #[must_use]
    pub fn chunk_chain(&self) -> RecordChunkChain {
        RecordChunkChain::new(self.predecessor.scope(), self.revision.id())
    }

    /// Confirms receipt of exactly the advertised source revision's chunks.
    ///
    /// Domain decoding and structural replay must also pass before publication.
    ///
    /// # Errors
    /// Rejects foreign, incomplete or altered chains.
    pub fn verify_chain(&self, chain: RecordChunkChain) -> Result<()> {
        if !chain.belongs_to(self.predecessor.scope(), self.revision.id()) {
            return Err(ProtocolError::new(
                FailureKind::ScopeMismatch,
                "chunk chain belongs to another source revision",
            ));
        }
        if chain.finish()? != self.chunks {
            return Err(ProtocolError::new(
                FailureKind::Integrity,
                "record chunk summary mismatch",
            ));
        }
        Ok(())
    }

    /// Computes this logical record's digest over its header and chained body.
    ///
    /// # Errors
    /// Rejects values that cannot be canonically encoded.
    pub fn record_digest(&self) -> Result<Digest> {
        self.unsigned_document()?.digest(DigestDomain::Record)
    }

    /// Returns the complete boundary resulting from this record.
    ///
    /// # Errors
    /// Rejects values that cannot be canonically encoded.
    pub fn head(&self) -> Result<Position> {
        Position::new(
            self.predecessor.scope(),
            Some(self.revision.id()),
            self.revision.sequence(),
            self.record_digest()?,
        )
    }

    /// Encodes the independently checked manifest and logical record digests.
    ///
    /// # Errors
    /// Rejects values that cannot be canonically encoded.
    pub fn document(&self) -> Result<Document> {
        let mut document = self.unsigned_document()?;
        let record_digest = document.digest(DigestDomain::Record)?;
        document.value["record_digest"] = record_digest.to_string().into();
        let digest = document.digest(DigestDomain::Manifest)?;
        document.value["digest"] = digest.to_string().into();
        Ok(document)
    }

    fn unsigned_document(&self) -> Result<Document> {
        Ok(Document {
            value: json!({
                "kind":"record.manifest","version":"1",
                "required_features":["metadata.v1","record-chunks.v1"],
                "predecessor":self.predecessor.document().value,
                "revision":encode_revision(&self.revision)?,
                "chunks":{"count":self.chunks.count().to_string(),
                    "payload_bytes":self.chunks.payload_bytes().to_string(),
                    "last_digest":self.chunks.last_digest().to_string()},
                "effects":self.effects.to_string(),"events":self.events.to_string(),
                "extensions":self.extensions.document().value
            }),
        })
    }
}
