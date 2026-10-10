//! Earlier evidence is transported exactly and never executed as effects.

use base64::{Engine, engine::general_purpose::STANDARD};
use postproject_core::RevisionId;
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, limit, malformed, nullable, object, text, unsupported},
};

/// Explicit archival families, distinct from replayable committed records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveFamily {
    /// A prior deterministic source floor anchor, including genesis.
    Anchor,
    /// Exact earlier manifest bytes, without a completeness claim.
    Manifest,
    /// One bounded fragment of an earlier encoded record chunk.
    Chunk,
    /// One bounded fragment of an earlier partial authored effect.
    PartialEffect,
}

impl ArchiveFamily {
    /// Returns the exact archive family tag.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::Manifest => "manifest",
            Self::Chunk => "chunk",
            Self::PartialEffect => "partial-effect",
        }
    }
}

/// One bounded piece of earlier history evidence with original coordinates.
///
/// Payloads remain opaque bytes. Storage checks the retained revision/floor and
/// original fragment order; decoding does not manufacture complete records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveEvidence {
    family: ArchiveFamily,
    revision: Option<RevisionId>,
    sequence: u64,
    position: u64,
    fragment: u64,
    payload: Vec<u8>,
}

impl ArchiveEvidence {
    /// Creates evidence bounded to one MiB, with checked original coordinates.
    ///
    /// # Errors
    /// Rejects empty/oversized bytes, non-storage ranges, missing revisions and
    /// impossible anchor/manifest coordinates. Evidence is not authenticated.
    pub fn new(
        family: ArchiveFamily,
        revision: Option<RevisionId>,
        sequence: u64,
        position: u64,
        fragment: u64,
        payload: Vec<u8>,
    ) -> Result<Self> {
        if payload.len() > 1_048_576 {
            return Err(limit());
        }
        if payload.is_empty()
            || [sequence, position, fragment]
                .into_iter()
                .any(|value| value > i64::MAX as u64)
            || (sequence == 0) != revision.is_none()
            || family != ArchiveFamily::Anchor && revision.is_none()
            || matches!(family, ArchiveFamily::Anchor | ArchiveFamily::Manifest)
                && (position != 0 || fragment != 0)
            || family == ArchiveFamily::Anchor && payload.len() != 32
        {
            return Err(malformed());
        }
        Ok(Self {
            family,
            revision,
            sequence,
            position,
            fragment,
            payload,
        })
    }

    /// Returns the archive family without interpreting the payload.
    #[must_use]
    pub const fn family(&self) -> ArchiveFamily {
        self.family
    }
    /// Returns the original revision, absent only for a genesis anchor.
    #[must_use]
    pub const fn revision(&self) -> Option<RevisionId> {
        self.revision
    }
    /// Returns the original revision/floor sequence.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Returns the original logical chunk/effect position.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.position
    }
    /// Returns the zero-based byte-fragment position within that item.
    #[must_use]
    pub const fn fragment(&self) -> u64 {
        self.fragment
    }
    /// Borrows the exact bytes, without making them executable.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Encodes bytes using canonical padded base64 and exact integer strings.
    #[must_use]
    pub fn document(&self) -> Document {
        Document {
            value: json!({"kind":"history.archive", "family":self.family.as_str(), "revision":self.revision.map(|id| id.to_string()), "sequence":self.sequence.to_string(), "position":self.position.to_string(), "fragment":self.fragment.to_string(), "payload":STANDARD.encode(&self.payload)}),
        }
    }

    /// Decodes bounded evidence, rejecting unknown tags/fields and inexact values.
    ///
    /// # Errors
    /// Rejects malformed coordinates, noncanonical base64 and excessive bytes.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind", "family", "revision", "sequence", "position", "fragment", "payload",
            ],
        )?;
        if document.kind()? != "history.archive" {
            return Err(unsupported());
        }
        let family = match text(&fields["family"])? {
            "anchor" => ArchiveFamily::Anchor,
            "manifest" => ArchiveFamily::Manifest,
            "chunk" => ArchiveFamily::Chunk,
            "partial-effect" => ArchiveFamily::PartialEffect,
            _ => return Err(unsupported()),
        };
        let encoded = text(&fields["payload"])?;
        if encoded.len() > 1_398_104 {
            return Err(limit());
        }
        let payload = STANDARD.decode(encoded).map_err(|_| malformed())?;
        if STANDARD.encode(&payload) != encoded {
            return Err(malformed());
        }
        Self::new(
            family,
            nullable(&fields["revision"], exact)?,
            exact(&fields["sequence"])?,
            exact(&fields["position"])?,
            exact(&fields["fragment"])?,
            payload,
        )
    }
}
