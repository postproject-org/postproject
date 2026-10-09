//! Resource headers are separate from independently streamed fingerprint facts.

use postproject_core::{FileFacts, Resource, ResourceId, Timestamp};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, nullable, object, text, unsupported},
};

/// Scalar resource identity and file facts; current/history fingerprints are separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceHeader {
    id: ResourceId,
    file_facts: Option<FileFacts>,
}

impl ResourceHeader {
    /// Copies scalar facts without materializing a fingerprint collection.
    #[must_use]
    pub const fn from_resource(resource: &Resource) -> Self {
        Self {
            id: resource.id(),
            file_facts: resource.file_facts(),
        }
    }

    /// Returns the original resource identity.
    #[must_use]
    pub const fn id(self) -> ResourceId {
        self.id
    }

    /// Returns exact stored observations, without new filesystem measurements.
    #[must_use]
    pub const fn file_facts(self) -> Option<FileFacts> {
        self.file_facts
    }

    /// Encodes a scalar body item; no fingerprint collection is implied.
    #[must_use]
    pub fn document(self) -> Document {
        let facts = self.file_facts.map(|facts| json!({"size_bytes":facts.size_bytes().to_string(), "modified_at_micros":facts.modified_at().map(|time| time.as_unix_micros().to_string())}));
        Document {
            value: json!({"kind":"resource.header", "id":self.id.to_string(), "file_facts":facts}),
        }
    }

    /// Decodes exact scalar facts; the importer checks owning aggregate references.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds and invalid identities or exact file facts.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "id", "file_facts"])?;
        if text(&fields["kind"])? != "resource.header" {
            return Err(unsupported());
        }
        let facts = nullable(&fields["file_facts"], |value| {
            let fields = object(value, &["size_bytes", "modified_at_micros"])?;
            Ok(FileFacts::new(
                exact(&fields["size_bytes"])?,
                nullable(&fields["modified_at_micros"], |value| {
                    Ok(Timestamp::from_unix_micros(exact(value)?))
                })?,
            ))
        })?;
        Ok(Self {
            id: exact(&fields["id"])?,
            file_facts: facts,
        })
    }
}
