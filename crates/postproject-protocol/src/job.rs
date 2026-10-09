//! Portable work observations contain no worker capability.

mod input;
mod state;
mod transition;
mod wire;
pub use input::JobInput;
pub use transition::{JobOperation, JobTransition};

use postproject_core::{Job, JobId, JobKind, JobState, MAX_JOB_INPUTS, RequestedJobOutput};

use crate::{
    Document, Result,
    fields::{malformed, unsupported},
};

/// Scalar job facts whose canonical input identities follow in separate frames.
///
/// A claimed state describes attribution and expiry. It contains no bearer
/// secret and grants no authority to execute or change the observed job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobHeader {
    id: JobId,
    kind: JobKind,
    inputs: u64,
    output: RequestedJobOutput,
    state: JobState,
}

impl JobHeader {
    /// Creates scalar observations within the native job-input count bound.
    ///
    /// # Errors
    /// Rejects excessive counts or future states without a wire definition.
    pub fn new(
        id: JobId,
        kind: JobKind,
        inputs: u64,
        output: RequestedJobOutput,
        state: JobState,
    ) -> Result<Self> {
        if usize::try_from(inputs)
            .ok()
            .is_none_or(|count| count > MAX_JOB_INPUTS)
        {
            return Err(malformed());
        }
        match &state {
            JobState::Requested
            | JobState::Claimed(_)
            | JobState::Succeeded(_)
            | JobState::Failed(_)
            | JobState::Cancelled => {}
            _ => return Err(unsupported()),
        }
        Ok(Self {
            id,
            kind,
            inputs,
            output,
            state,
        })
    }

    /// Copies scalar observations without cloning the native input collection.
    ///
    /// # Errors
    /// Rejects excessive counts or unsupported future lifecycle alternatives.
    pub fn from_job(job: &Job) -> Result<Self> {
        Self::new(
            job.id(),
            job.kind().clone(),
            u64::try_from(job.inputs().len()).map_err(|_| malformed())?,
            job.requested_output().clone(),
            job.state().clone(),
        )
    }

    /// Returns the original stable job identity.
    #[must_use]
    pub const fn id(&self) -> JobId {
        self.id
    }

    /// Returns the exact open-world work kind.
    #[must_use]
    pub const fn kind(&self) -> &JobKind {
        &self.kind
    }

    /// Returns the number of following canonical input identities.
    #[must_use]
    pub const fn input_count(&self) -> u64 {
        self.inputs
    }

    /// Returns the requested owning asset, representation role and logical root.
    #[must_use]
    pub const fn requested_output(&self) -> &RequestedJobOutput {
        &self.output
    }

    /// Returns observational lifecycle detail, without an executable credential.
    #[must_use]
    pub const fn state(&self) -> &JobState {
        &self.state
    }

    /// Encodes complete scalar observations without an input aggregate.
    ///
    /// # Errors
    /// Rejects future representation or lifecycle alternatives lacking a codec.
    pub fn document(&self) -> Result<Document> {
        wire::encode(self)
    }

    /// Decodes through the same checked values used by native work operations.
    ///
    /// # Errors
    /// Rejects unknown fields, invalid attribution and malformed exact values.
    pub fn from_document(document: &Document) -> Result<Self> {
        wire::decode(document)
    }
}
