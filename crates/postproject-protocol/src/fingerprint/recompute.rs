use postproject_core::{RepresentationId, ResourceId};
use serde_json::json;

use super::revision_sequence;
use crate::{
    Document, Result,
    fields::{exact, object, text, unsupported},
};

/// Recorded need to recompute representation evidence after a resource changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FingerprintRecomputation {
    representation: RepresentationId,
    resource: ResourceId,
    sequence: u64,
}

impl FingerprintRecomputation {
    /// Records original identifiers and the revision that marked recomputation.
    ///
    /// # Errors
    /// Rejects zero or out-of-storage-range revision sequences.
    pub fn new(
        representation: RepresentationId,
        resource: ResourceId,
        sequence: u64,
    ) -> Result<Self> {
        Ok(Self {
            representation,
            resource,
            sequence: revision_sequence(sequence)?,
        })
    }

    /// Returns the representation whose evidence needs recomputation.
    #[must_use]
    pub const fn representation_id(self) -> RepresentationId {
        self.representation
    }

    /// Returns the resource change recorded by the authority.
    #[must_use]
    pub const fn changed_resource_id(self) -> ResourceId {
        self.resource
    }

    /// Returns the original marking revision sequence.
    #[must_use]
    pub const fn revision_sequence(self) -> u64 {
        self.sequence
    }

    /// Encodes a recorded marker without evaluating or recomputing fingerprints.
    #[must_use]
    pub fn document(self) -> Document {
        Document {
            value: json!({"kind":"fingerprint.recomputation", "representation_id":self.representation.to_string(), "changed_resource_id":self.resource.to_string(), "revision_sequence":self.sequence.to_string()}),
        }
    }

    /// Decodes a recorded marker; storage verifies references and revision ownership.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds and invalid original identity/revision values.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "representation_id",
                "changed_resource_id",
                "revision_sequence",
            ],
        )?;
        if text(&fields["kind"])? != "fingerprint.recomputation" {
            return Err(unsupported());
        }
        Self::new(
            exact(&fields["representation_id"])?,
            exact(&fields["changed_resource_id"])?,
            exact(&fields["revision_sequence"])?,
        )
    }
}
