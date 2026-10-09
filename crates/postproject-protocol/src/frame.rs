//! Incremental canonical documents across independently bounded chunk payloads.

use crate::{Document, FailureKind, Limits, ProtocolError, Result};

/// Decodes length-prefixed body documents while retaining at most one frame.
///
/// The caller verifies the owning chunk chain separately and supplies explicit
/// per-document limits. A document can cross any number of chunk boundaries.
/// Dropping the decoder cancels processing without publishing any domain state.
pub struct FrameDecoder {
    limits: Limits,
    header: [u8; 8],
    header_bytes: usize,
    length: Option<usize>,
    bytes: Vec<u8>,
    closed: bool,
}

impl FrameDecoder {
    /// Starts a decoder with checked per-document memory/work limits.
    #[must_use]
    pub const fn new(limits: Limits) -> Self {
        Self {
            limits,
            header: [0; 8],
            header_bytes: 0,
            length: None,
            bytes: Vec::new(),
            closed: false,
        }
    }

    /// Consumes a prefix of `input`, yielding at most one complete document.
    ///
    /// Advance by the returned count and call again for any remaining bytes.
    /// A nonempty input always makes progress. An empty input yields `(0, None)`.
    ///
    /// # Errors
    /// Rejects excessive lengths before allocating, noncanonical/invalid JSON,
    /// or calls after completion/failure. Every failure closes this decoder.
    pub fn consume(&mut self, input: &[u8]) -> Result<(usize, Option<Document>)> {
        if self.closed {
            return Err(invalid());
        }
        let result = self.consume_open(input);
        if result.is_err() {
            self.closed = true;
            self.bytes = Vec::new();
        }
        result
    }

    fn consume_open(&mut self, input: &[u8]) -> Result<(usize, Option<Document>)> {
        let mut consumed = 0;
        if self.length.is_none() {
            let count = input.len().min(8 - self.header_bytes);
            self.header[self.header_bytes..self.header_bytes + count]
                .copy_from_slice(&input[..count]);
            self.header_bytes += count;
            consumed += count;
            if self.header_bytes != 8 {
                return Ok((consumed, None));
            }
            let length = usize::try_from(u64::from_be_bytes(self.header)).map_err(|_| limit())?;
            if length == 0 || length > self.limits.max_bytes() {
                return Err(limit());
            }
            self.bytes.try_reserve_exact(length).map_err(|_| limit())?;
            self.length = Some(length);
        }
        let length = self.length.ok_or_else(invalid)?;
        let count = (input.len() - consumed).min(length - self.bytes.len());
        self.bytes
            .extend_from_slice(&input[consumed..consumed + count]);
        consumed += count;
        if self.bytes.len() != length {
            return Ok((consumed, None));
        }
        let document = Document::parse(&self.bytes, self.limits)?;
        if document.canonical_bytes()? != self.bytes {
            return Err(invalid());
        }
        self.bytes = Vec::new();
        self.header_bytes = 0;
        self.length = None;
        Ok((consumed, Some(document)))
    }

    /// Closes the stream, verifying that no partial frame remains.
    ///
    /// # Errors
    /// Rejects truncated lengths/documents or an already closed decoder.
    pub fn finish(&mut self) -> Result<()> {
        let valid = !self.closed && self.header_bytes == 0 && self.length.is_none();
        self.closed = true;
        self.bytes = Vec::new();
        if valid { Ok(()) } else { Err(invalid()) }
    }
}

fn invalid() -> ProtocolError {
    ProtocolError::new(FailureKind::Integrity, "invalid canonical record framing")
}
fn limit() -> ProtocolError {
    ProtocolError::new(
        FailureKind::LimitExceeded,
        "record frame exceeds caller budget",
    )
}
