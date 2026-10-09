use crate::{
    CheckpointChunk, CheckpointId, CheckpointSection, ChunkSummary, Digest, FailureKind,
    ProtocolError, Result, Scope, fields::limit,
};

/// Constant-space verifier for exactly one checkpoint section's ordered bytes.
#[derive(Clone, Debug)]
pub struct CheckpointChunkChain {
    scope: Scope,
    checkpoint: CheckpointId,
    section: CheckpointSection,
    count: u64,
    bytes: u64,
    last: Option<Digest>,
}

impl CheckpointChunkChain {
    /// Starts one explicitly scoped section; empty sections emit no chunks.
    #[must_use]
    pub const fn new(scope: Scope, checkpoint: CheckpointId, section: CheckpointSection) -> Self {
        Self {
            scope,
            checkpoint,
            section,
            count: 0,
            bytes: 0,
            last: None,
        }
    }

    pub(crate) fn belongs_to(
        &self,
        scope: Scope,
        checkpoint: CheckpointId,
        section: CheckpointSection,
    ) -> bool {
        self.scope == scope && self.checkpoint == checkpoint && self.section == section
    }

    /// Verifies one chunk; a failure leaves the accumulated boundary unchanged.
    ///
    /// # Errors
    /// Rejects wrong owners/sections, reordered/missing chunks, altered
    /// predecessors and count/byte overflow.
    pub fn push(&mut self, chunk: &CheckpointChunk) -> Result<()> {
        if !self.belongs_to(chunk.scope(), chunk.checkpoint(), chunk.section()) {
            return Err(ProtocolError::new(
                FailureKind::ScopeMismatch,
                "checkpoint chunk belongs to another export or section",
            ));
        }
        if chunk.index() != self.count {
            return Err(ProtocolError::new(
                FailureKind::HistoryGap,
                "checkpoint chunk is missing or out of order",
            ));
        }
        if chunk.previous() != self.last {
            return Err(ProtocolError::new(
                FailureKind::Integrity,
                "checkpoint chunk predecessor mismatch",
            ));
        }
        let count = self
            .count
            .checked_add(1)
            .filter(|count| i64::try_from(*count).is_ok())
            .ok_or_else(limit)?;
        let bytes = self
            .bytes
            .checked_add(u64::try_from(chunk.payload().len()).map_err(|_| limit())?)
            .ok_or_else(limit)?;
        let digest = chunk.digest()?;
        self.count = count;
        self.bytes = bytes;
        self.last = Some(digest);
        Ok(())
    }

    /// Returns the exact completed commitment for a nonempty section.
    ///
    /// # Errors
    /// Rejects missing chunks. Compare the result with the manifest descriptor
    /// before considering this section complete; domain decoding is separate.
    pub fn finish(self) -> Result<ChunkSummary> {
        ChunkSummary::new(
            self.count,
            self.bytes,
            self.last.ok_or_else(crate::fields::malformed)?,
        )
    }
}
