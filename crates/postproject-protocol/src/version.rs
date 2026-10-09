//! Semantic property versions carry domain keys rather than backend encodings.

use postproject_core::{RevisionId, SemanticConflictKey};
use serde_json::json;

use crate::{
    Document, Result,
    conflict::{decode_key, encode_key},
    fields::{exact, malformed, object, text, unsupported},
};

/// Original revision boundary of a semantic conflict key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConflictVersion {
    key: SemanticConflictKey,
    revision: RevisionId,
    sequence: u64,
}

impl ConflictVersion {
    /// Constructs a non-genesis property version using a supported semantic key.
    ///
    /// # Errors
    /// Rejects zero/out-of-range sequences and unsupported or invalid keys.
    pub fn new(key: SemanticConflictKey, revision: RevisionId, sequence: u64) -> Result<Self> {
        if sequence == 0 || i64::try_from(sequence).is_err() {
            return Err(malformed());
        }
        encode_key(&key)?;
        Ok(Self {
            key,
            revision,
            sequence,
        })
    }

    /// Returns the exact domain conflict key.
    #[must_use]
    pub const fn key(&self) -> &SemanticConflictKey {
        &self.key
    }

    /// Returns the original authoritative revision identity.
    #[must_use]
    pub const fn revision(&self) -> RevisionId {
        self.revision
    }

    /// Returns its original sequence; the importer checks retained history.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Encodes a semantic version without exposing private SQLite key bytes.
    ///
    /// # Errors
    /// Rejects unsupported semantic alternatives.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: json!({"kind":"conflict.version", "key":encode_key(&self.key)?, "revision":self.revision.to_string(), "sequence":self.sequence.to_string()}),
        })
    }

    /// Decodes checked fields; this does not prove target/history existence.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds and malformed keys or boundaries.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "key", "revision", "sequence"])?;
        if text(&fields["kind"])? != "conflict.version" {
            return Err(unsupported());
        }
        Self::new(
            decode_key(&fields["key"])?,
            exact(&fields["revision"])?,
            exact(&fields["sequence"])?,
        )
    }
}
