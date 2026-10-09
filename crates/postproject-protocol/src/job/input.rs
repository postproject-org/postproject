use postproject_core::{JobId, MAX_JOB_INPUTS, RepresentationId};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, malformed, object, unsupported},
};

/// One canonical input identity, separate from job attribution and state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobInput {
    job: JobId,
    position: u64,
    representation: RepresentationId,
}

impl JobInput {
    /// Creates a position within the existing native job-input aggregate bound.
    ///
    /// # Errors
    /// Rejects positions that cannot belong to a valid native job.
    pub fn new(job: JobId, position: u64, representation: RepresentationId) -> Result<Self> {
        if usize::try_from(position)
            .ok()
            .is_none_or(|position| position >= MAX_JOB_INPUTS)
        {
            return Err(malformed());
        }
        Ok(Self {
            job,
            position,
            representation,
        })
    }

    /// Returns the owning work request's identity.
    #[must_use]
    pub const fn job_id(self) -> JobId {
        self.job
    }

    /// Returns the original zero-based canonical input position.
    #[must_use]
    pub const fn position(self) -> u64 {
        self.position
    }

    /// Returns the representation consumed by the request.
    #[must_use]
    pub const fn representation_id(self) -> RepresentationId {
        self.representation
    }

    /// Encodes one identity without materializing the full request collection.
    #[must_use]
    pub fn document(self) -> Document {
        Document {
            value: json!({"kind":"job.input", "job_id":self.job.to_string(), "position":self.position.to_string(), "representation_id":self.representation.to_string()}),
        }
    }

    /// Decodes a typed input identity with an exact original position.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds, invalid identities and excessive positions.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &["kind", "job_id", "position", "representation_id"],
        )?;
        if document.kind()? != "job.input" {
            return Err(unsupported());
        }
        Self::new(
            exact(&fields["job_id"])?,
            exact(&fields["position"])?,
            exact(&fields["representation_id"])?,
        )
    }
}
