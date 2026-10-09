//! Compact recovery facts; complete attribution and diagnostics stay in records.

use postproject_core::{JobCompletion, JobId, JobState, Timestamp};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, malformed, object, text, unsupported},
};

/// Final lifecycle observations retained in a submission result.
///
/// These summaries grant no ownership. Full request, attribution, failure and
/// input evidence is available in the receipt's committed record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum JobResultState {
    /// Waiting for a worker.
    Requested,
    /// Observed authority-selected expiry, without attribution or a secret.
    Claimed(Timestamp),
    /// Atomically published activity and representation identities.
    Succeeded(JobCompletion),
    /// Failed; the exact diagnostic is retained in committed effects.
    Failed,
    /// Administratively cancelled.
    Cancelled,
}

/// One affected job's final state at this submission's commit boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobResult {
    job: JobId,
    state: JobResultState,
}

impl JobResult {
    /// Creates a noncredential summary with explicit lifecycle alternatives.
    #[must_use]
    pub const fn new(job: JobId, state: JobResultState) -> Self {
        Self { job, state }
    }

    /// Copies recovery facts from native observed state without worker ownership.
    ///
    /// # Errors
    /// Rejects future lifecycle alternatives without a wire definition.
    pub fn from_state(job: JobId, state: &JobState) -> Result<Self> {
        Ok(Self::new(
            job,
            match state {
                JobState::Requested => JobResultState::Requested,
                JobState::Claimed(claim) => JobResultState::Claimed(claim.expires_at()),
                JobState::Succeeded(completion) => JobResultState::Succeeded(*completion),
                JobState::Failed(_) => JobResultState::Failed,
                JobState::Cancelled => JobResultState::Cancelled,
                _ => return Err(unsupported()),
            },
        ))
    }

    /// Returns the affected stable job identity.
    #[must_use]
    pub const fn job_id(self) -> JobId {
        self.job
    }

    /// Returns observations at the original commit, never the latest state.
    #[must_use]
    pub const fn state(self) -> JobResultState {
        self.state
    }

    /// Encodes exact recovery facts, without a diagnostic or credential field.
    #[must_use]
    pub fn document(self) -> Document {
        let state = match self.state {
            JobResultState::Requested => json!({"kind":"requested"}),
            JobResultState::Claimed(expiry) => {
                json!({"kind":"claimed", "expires_at_micros":expiry.as_unix_micros().to_string()})
            }
            JobResultState::Succeeded(completion) => {
                json!({"kind":"succeeded", "activity_id":completion.activity_id().to_string(), "representation_id":completion.representation_id().to_string()})
            }
            JobResultState::Failed => json!({"kind":"failed"}),
            JobResultState::Cancelled => json!({"kind":"cancelled"}),
        };
        Document {
            value: json!({"job_id":self.job.to_string(), "state":state}),
        }
    }

    /// Decodes strict scalar observations without authorizing execution.
    ///
    /// # Errors
    /// Rejects extra fields, unknown states or malformed exact values.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["job_id", "state"])?;
        let state = &fields["state"];
        let state = match text(state.get("kind").ok_or_else(malformed)?)? {
            "requested" | "failed" | "cancelled" => {
                let fields = object(state, &["kind"])?;
                match text(&fields["kind"])? {
                    "requested" => JobResultState::Requested,
                    "failed" => JobResultState::Failed,
                    _ => JobResultState::Cancelled,
                }
            }
            "claimed" => {
                let fields = object(state, &["kind", "expires_at_micros"])?;
                JobResultState::Claimed(Timestamp::from_unix_micros(exact(
                    &fields["expires_at_micros"],
                )?))
            }
            "succeeded" => {
                let fields = object(state, &["kind", "activity_id", "representation_id"])?;
                JobResultState::Succeeded(JobCompletion::new(
                    exact(&fields["activity_id"])?,
                    exact(&fields["representation_id"])?,
                ))
            }
            _ => return Err(unsupported()),
        };
        Ok(Self::new(exact(&fields["job_id"])?, state))
    }
}
