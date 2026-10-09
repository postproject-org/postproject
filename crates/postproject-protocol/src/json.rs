//! Bounded JSON framing, with duplicate keys checked before map insertion.

mod bounds;
mod decode;
mod encode;

use serde::de::DeserializeSeed;
use serde_json::Value;

use crate::{Digest, DigestDomain, FailureKind, ProtocolError, Result};

/// Maximum checked container depth, allowing all 32 levels of domain metadata.
pub const MAX_CONTAINER_DEPTH: usize = 192;

/// Checked per-document resource limits, separate from domain value limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub(crate) bytes: usize,
    pub(crate) depth: usize,
    pub(crate) nodes: usize,
}

impl Limits {
    /// Creates positive limits with a container depth no greater than 192.
    ///
    /// # Errors
    /// Rejects zero limits or a depth above the parser's safe supported bound.
    pub fn new(bytes: usize, depth: usize, nodes: usize) -> Result<Self> {
        if bytes == 0 || nodes == 0 || !(1..=MAX_CONTAINER_DEPTH).contains(&depth) {
            return Err(ProtocolError::new(
                FailureKind::LimitExceeded,
                "invalid JSON resource limits",
            ));
        }
        Ok(Self {
            bytes,
            depth,
            nodes,
        })
    }

    /// Returns the maximum encoded document length.
    #[must_use]
    pub const fn max_bytes(self) -> usize {
        self.bytes
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            depth: MAX_CONTAINER_DEPTH,
            nodes: 1_000_000,
        }
    }
}

/// Strict JSON framing; this does not authorize a domain command or effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    pub(crate) value: Value,
}

impl Document {
    /// Returns a document's string kind for domain-decoder dispatch.
    ///
    /// This does not validate or authorize the remaining fields.
    ///
    /// # Errors
    /// Rejects a missing or nonstring kind.
    pub fn kind(&self) -> Result<&str> {
        self.value
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(crate::fields::malformed)
    }

    /// Encodes canonical JSON, retaining all fields and array order.
    ///
    /// # Errors
    /// Returns malformed if an internal value cannot use the exact wire profile.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        encode::canonical(&self.value, false)
    }

    pub(crate) fn bounded_canonical_bytes(&self, limits: Limits) -> Result<Vec<u8>> {
        bounds::validate(&self.value, limits)?;
        let bytes = self.canonical_bytes()?;
        if bytes.len() > limits.max_bytes() {
            return Err(crate::fields::limit());
        }
        Ok(bytes)
    }

    /// Computes integrity, omitting only the object's own top-level `digest`.
    ///
    /// This validates JSON framing, not the field's domain semantics.
    ///
    /// # Errors
    /// Returns malformed for an internally unencodable value.
    pub fn digest(&self, domain: DigestDomain) -> Result<Digest> {
        Ok(Digest::of_canonical_bytes(
            domain,
            &encode::canonical(&self.value, true)?,
        ))
    }

    /// Parses a bounded object, rejecting duplicate keys and JSON number tokens.
    ///
    /// # Errors
    /// Returns a typed malformed/limit failure without echoing input contents.
    pub fn parse(bytes: &[u8], limits: Limits) -> Result<Self> {
        if bytes.len() > limits.bytes {
            return Err(ProtocolError::new(
                FailureKind::LimitExceeded,
                "document exceeds byte limit",
            ));
        }
        let state = decode::State::new(limits);
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        // Tagged struct fields add several wire containers per domain level.
        // Our seed bounds every container before entering it, including when
        // serde_json's smaller default would reject legal domain metadata.
        decoder.disable_recursion_limit();
        let value = decode::Seed::new(&state)
            .deserialize(&mut decoder)
            .and_then(|value| decoder.end().map(|()| value))
            .map_err(|_| ProtocolError::new(state.failure(), "invalid bounded JSON document"))?;
        if !value.is_object() {
            return Err(ProtocolError::new(
                FailureKind::Malformed,
                "document must be an object",
            ));
        }
        Ok(Self { value })
    }
}
