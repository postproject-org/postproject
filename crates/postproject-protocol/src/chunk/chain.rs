//! Constant-space verification of a complete ordered record-chunk chain.

use postproject_core::RevisionId;

use crate::{
    Digest, FailureKind, MAX_RECORD_CHUNK_PAYLOAD, ProtocolError, RecordChunk, Result, Scope,
    fields::{limit, malformed},
};

/// Checked counts and the final chained digest of a complete record body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkSummary {
    count: u64,
    payload_bytes: u64,
    last_digest: Digest,
}

impl ChunkSummary {
    /// Validates summary bounds without asserting that its chunks were received.
    ///
    /// # Errors
    /// Rejects an empty body, impossible byte counts or out-of-range counts.
    pub fn new(count: u64, payload_bytes: u64, last_digest: Digest) -> Result<Self> {
        let maximum =
            u128::from(count) * u128::try_from(MAX_RECORD_CHUNK_PAYLOAD).map_err(|_| limit())?;
        if count == 0
            || count > i64::MAX as u64
            || payload_bytes < count
            || u128::from(payload_bytes) > maximum
        {
            return Err(malformed());
        }
        Ok(Self {
            count,
            payload_bytes,
            last_digest,
        })
    }
    /// Returns the exact number of ordered chunks.
    #[must_use]
    pub const fn count(self) -> u64 {
        self.count
    }
    /// Returns total reconstructed bytes, excluding chunk envelopes.
    #[must_use]
    pub const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }
    /// Returns the digest binding every predecessor in the complete chain.
    #[must_use]
    pub const fn last_digest(self) -> Digest {
        self.last_digest
    }
}

/// Verifies ordered chunks without retaining or materializing their payloads.
///
/// A caller may spool bytes privately while this checks framing. Its final
/// summary must match the record manifest before domain replay becomes visible.
#[derive(Clone, Debug)]
pub struct RecordChunkChain {
    scope: Scope,
    revision: RevisionId,
    count: u64,
    payload_bytes: u64,
    last_digest: Option<Digest>,
}

impl RecordChunkChain {
    pub(crate) fn belongs_to(&self, scope: Scope, revision: RevisionId) -> bool {
        self.scope == scope && self.revision == revision
    }

    /// Starts verification for one explicitly advertised source revision.
    #[must_use]
    pub const fn new(scope: Scope, revision: RevisionId) -> Self {
        Self {
            scope,
            revision,
            count: 0,
            payload_bytes: 0,
            last_digest: None,
        }
    }

    /// Checks one continuation; failure leaves the accumulated boundary intact.
    ///
    /// # Errors
    /// Rejects foreign source/revision, missing/reordered chunks, altered
    /// predecessors or byte/count overflow.
    pub fn push(&mut self, chunk: &RecordChunk) -> Result<()> {
        if chunk.scope() != self.scope || chunk.revision() != self.revision {
            return Err(ProtocolError::new(
                FailureKind::ScopeMismatch,
                "record chunk belongs to another source revision",
            ));
        }
        if chunk.index() != self.count {
            return Err(ProtocolError::new(
                FailureKind::HistoryGap,
                "record chunk is missing or out of order",
            ));
        }
        if chunk.previous() != self.last_digest {
            return Err(ProtocolError::new(
                FailureKind::Integrity,
                "record chunk predecessor mismatch",
            ));
        }
        let count = self
            .count
            .checked_add(1)
            .filter(|count| i64::try_from(*count).is_ok())
            .ok_or_else(limit)?;
        let bytes = u64::try_from(chunk.payload().len()).map_err(|_| limit())?;
        let payload_bytes = self.payload_bytes.checked_add(bytes).ok_or_else(limit)?;
        let digest = chunk.digest()?;
        self.count = count;
        self.payload_bytes = payload_bytes;
        self.last_digest = Some(digest);
        Ok(())
    }

    /// Finishes framing verification; a manifest comparison is still required.
    ///
    /// # Errors
    /// Rejects missing chunks; no-op outcomes have no record body.
    pub fn finish(self) -> Result<ChunkSummary> {
        ChunkSummary::new(
            self.count,
            self.payload_bytes,
            self.last_digest.ok_or_else(malformed)?,
        )
    }
}
