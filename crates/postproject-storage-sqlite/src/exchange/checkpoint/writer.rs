//! One section buffer; publication of the final manifest belongs to the caller.

use postproject_protocol::{
    CheckpointChunk, CheckpointChunkChain, CheckpointId, CheckpointSection, Digest, Document,
    Extensions, Scope, SectionSummary,
};

use crate::ExchangeResult;

const PAYLOAD_BYTES: usize = 1024 * 1024;

pub(super) struct SectionWriter<'a, Sink> {
    scope: Scope,
    checkpoint: CheckpointId,
    section: CheckpointSection,
    chain: CheckpointChunkChain,
    previous: Option<Digest>,
    index: u64,
    items: u64,
    buffer: Vec<u8>,
    sink: &'a mut Sink,
}

impl<'a, Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>> SectionWriter<'a, Sink> {
    pub(super) fn new(
        scope: Scope,
        checkpoint: CheckpointId,
        section: CheckpointSection,
        sink: &'a mut Sink,
    ) -> Self {
        Self {
            scope,
            checkpoint,
            section,
            chain: CheckpointChunkChain::new(scope, checkpoint, section),
            previous: None,
            index: 0,
            items: 0,
            buffer: Vec::with_capacity(PAYLOAD_BYTES),
            sink,
        }
    }

    pub(super) fn document(
        &mut self,
        document: &Document,
        starts_item: bool,
    ) -> ExchangeResult<()> {
        let bytes = document.canonical_bytes()?;
        self.bytes(
            &u64::try_from(bytes.len())
                .map_err(|_| super::invalid())?
                .to_be_bytes(),
        )?;
        self.bytes(&bytes)?;
        if starts_item {
            self.items = self.items.checked_add(1).ok_or_else(super::invalid)?;
        }
        Ok(())
    }

    fn bytes(&mut self, mut bytes: &[u8]) -> ExchangeResult<()> {
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

    fn flush(&mut self) -> ExchangeResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let chunk = CheckpointChunk::new(
            self.scope,
            self.checkpoint,
            self.section,
            self.index,
            self.previous,
            std::mem::replace(&mut self.buffer, Vec::with_capacity(PAYLOAD_BYTES)),
            Extensions::default(),
        )?;
        self.chain.push(&chunk)?;
        self.previous = Some(chunk.digest()?);
        (self.sink)(chunk)?;
        self.index = self.index.checked_add(1).ok_or_else(super::invalid)?;
        Ok(())
    }

    pub(super) fn finish(mut self) -> ExchangeResult<SectionSummary> {
        self.flush()?;
        let chunks = if self.items == 0 {
            None
        } else {
            Some(self.chain.finish()?)
        };
        Ok(SectionSummary::new(self.section, self.items, chunks)?)
    }
}
