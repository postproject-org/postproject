//! Domain-separated BLAKE3 integrity values, without an authentication claim.

use std::{fmt, str::FromStr};

use crate::{FailureKind, ProtocolError, Result};

/// The exact purpose of a canonical protocol digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestDomain {
    /// Normalized complete proposal identity.
    Request,
    /// Complete logical committed record.
    Record,
    /// Bounded stream chunk.
    Chunk,
    /// Checkpoint or logical-record manifest.
    Manifest,
    /// Persistent genesis or migration replay floor.
    Anchor,
    /// Canonical portable state comparison.
    State,
}

impl DigestDomain {
    /// Returns the exact BLAKE3 derive-key context.
    #[must_use]
    pub const fn context(self) -> &'static str {
        match self {
            Self::Request => "postproject.exchange.v1.request",
            Self::Record => "postproject.exchange.v1.record",
            Self::Chunk => "postproject.exchange.v1.chunk",
            Self::Manifest => "postproject.exchange.v1.manifest",
            Self::Anchor => "postproject.exchange.v1.anchor",
            Self::State => "postproject.exchange.v1.state",
        }
    }
}

/// A 32-byte integrity digest with canonical lowercase hex text.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Digest([u8; 32]);

impl Digest {
    /// Reconstructs exact bytes from validated storage or an integrity manifest.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Hashes canonical bytes in the specified protocol domain.
    #[must_use]
    pub fn of_canonical_bytes(domain: DigestDomain, bytes: &[u8]) -> Self {
        Self(blake3::derive_key(domain.context(), bytes))
    }

    /// Returns the stable bytes for storage or comparison.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for Digest {
    type Err = ProtocolError;

    fn from_str(text: &str) -> Result<Self> {
        if text.len() != 64
            || !text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ProtocolError::new(
                FailureKind::Malformed,
                "noncanonical digest",
            ));
        }
        let mut bytes = [0; 32];
        for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            let digit = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
            *byte = digit(pair[0]) * 16 + digit(pair[1]);
        }
        Ok(Self(bytes))
    }
}
