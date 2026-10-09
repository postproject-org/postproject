//! Complete dependency observations, streamed in authored occurrence order.

mod occurrence;
pub use occurrence::DependencyOccurrence;

use postproject_core::{
    DependencySet, DependencySetStatus, MAX_DEPENDENCIES_PER_SET, RepresentationId,
};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, malformed, object, text, unsupported},
    fingerprint::revision_sequence,
};

/// A complete dependency-set header, without collecting its occurrence bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencySetHeader {
    source: RepresentationId,
    sequence: u64,
    status: DependencySetStatus,
    count: u64,
}

impl DependencySetHeader {
    /// Creates a header retaining original revision and extraction status.
    ///
    /// # Errors
    /// Rejects zero/out-of-range revisions, future status alternatives and counts
    /// exceeding the existing core dependency-set bound.
    pub fn new(
        source: RepresentationId,
        sequence: u64,
        status: DependencySetStatus,
        count: u64,
    ) -> Result<Self> {
        if usize::try_from(count)
            .ok()
            .is_none_or(|count| count > MAX_DEPENDENCIES_PER_SET)
        {
            return Err(malformed());
        }
        match status {
            DependencySetStatus::Current | DependencySetStatus::NeedsExtraction => {}
            _ => return Err(unsupported()),
        }
        Ok(Self {
            source,
            sequence: revision_sequence(sequence)?,
            status,
            count,
        })
    }

    /// Extracts a header while borrowing the already validated domain set.
    ///
    /// # Errors
    /// Rejects unsupported statuses or invalid portable revision boundaries.
    pub fn from_set(set: &DependencySet) -> Result<Self> {
        Self::new(
            set.source_representation_id(),
            set.recorded_at_revision(),
            set.status(),
            u64::try_from(set.dependencies().len()).map_err(|_| malformed())?,
        )
    }

    /// Returns the representation whose content supplied this observation.
    #[must_use]
    pub const fn source_representation_id(self) -> RepresentationId {
        self.source
    }

    /// Returns the original revision that recorded the set.
    #[must_use]
    pub const fn recorded_at_revision(self) -> u64 {
        self.sequence
    }

    /// Returns currentness or the recorded need for extraction.
    #[must_use]
    pub const fn status(self) -> DependencySetStatus {
        self.status
    }

    /// Returns the exact number of following ordered occurrence frames.
    #[must_use]
    pub const fn occurrence_count(self) -> u64 {
        self.count
    }

    /// Encodes the scalar header, including complete empty observations.
    #[must_use]
    pub fn document(self) -> Document {
        let status = match self.status {
            DependencySetStatus::Current => "current",
            DependencySetStatus::NeedsExtraction => "needs-extraction",
            _ => unreachable!("checked dependency status"),
        };
        Document {
            value: json!({"kind":"dependency.set", "source_representation_id":self.source.to_string(), "recorded_revision_sequence":self.sequence.to_string(), "status":status, "occurrence_count":self.count.to_string()}),
        }
    }

    /// Decodes checked scalar facts; storage validates ownership and completeness.
    ///
    /// # Errors
    /// Rejects unknown kinds/fields/statuses and invalid revisions/counts.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "source_representation_id",
                "recorded_revision_sequence",
                "status",
                "occurrence_count",
            ],
        )?;
        if document.kind()? != "dependency.set" {
            return Err(unsupported());
        }
        let status = match text(&fields["status"])? {
            "current" => DependencySetStatus::Current,
            "needs-extraction" => DependencySetStatus::NeedsExtraction,
            _ => return Err(unsupported()),
        };
        Self::new(
            exact(&fields["source_representation_id"])?,
            exact(&fields["recorded_revision_sequence"])?,
            status,
            exact(&fields["occurrence_count"])?,
        )
    }
}
