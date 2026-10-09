use std::time::Duration;

use postproject_core::{
    JobId, JobState, RevisionEventKind, Timestamp, validate_job_lease_duration,
};
use serde_json::json;

use super::state;
use crate::{
    Document, Result,
    fields::{checked, exact, malformed, nullable, object, text, unsupported},
};

/// One authored job lifecycle operation, without its private capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobOperation {
    /// Claim requested or expired work through the authority.
    Claim,
    /// Extend current ownership.
    Renew,
    /// Release current ownership.
    Release,
    /// Record a terminal worker failure.
    Fail,
    /// Publish the requested output and provenance.
    Complete,
    /// Administratively cancel nonterminal work.
    Cancel,
}

impl JobOperation {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Claim => "claim",
            Self::Renew => "renew",
            Self::Release => "release",
            Self::Fail => "fail",
            Self::Complete => "complete",
            Self::Cancel => "cancel",
        }
    }

    fn decode(value: &str) -> Result<Self> {
        Ok(match value {
            "claim" => Self::Claim,
            "renew" => Self::Renew,
            "release" => Self::Release,
            "fail" => Self::Fail,
            "complete" => Self::Complete,
            "cancel" => Self::Cancel,
            _ => return Err(unsupported()),
        })
    }
}

/// Historical state and authority decisions at an original operation boundary.
///
/// Receivers validate these facts without consulting their local wall clock.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobTransition {
    job: JobId,
    operation: JobOperation,
    previous: JobState,
    state: JobState,
    authority_time: Option<Timestamp>,
    input_boundary: Option<u64>,
}

impl JobTransition {
    /// Checks the original state transition and exact recorded authority time.
    ///
    /// The input boundary is the sequence used by the original claim/completion
    /// decision. Zero identifies a genesis decision; it is never a credential.
    ///
    /// # Errors
    /// Rejects impossible lifecycle transitions or inconsistent decision fields.
    pub fn new(
        job: JobId,
        operation: JobOperation,
        previous: JobState,
        state: JobState,
        authority_time: Option<Timestamp>,
        input_boundary: Option<u64>,
    ) -> Result<Self> {
        if input_boundary.is_some_and(|value| value > i64::MAX as u64)
            || matches!(operation, JobOperation::Claim | JobOperation::Complete)
                != input_boundary.is_some()
            || (operation == JobOperation::Cancel) == authority_time.is_some()
        {
            return Err(malformed());
        }
        let now = authority_time.map(Timestamp::as_unix_micros);
        if matches!(operation, JobOperation::Claim | JobOperation::Renew) {
            let JobState::Claimed(claim) = &state else {
                return Err(malformed());
            };
            let micros = claim
                .expires_at()
                .as_unix_micros()
                .checked_sub(now.ok_or_else(malformed)?)
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(malformed)?;
            checked(validate_job_lease_duration(Duration::from_micros(micros)))?;
        }
        let valid = match (operation, &previous, &state) {
            (JobOperation::Claim, old, JobState::Claimed(new)) => {
                let now = now.ok_or_else(malformed)?;
                (matches!(old, JobState::Requested)
                    || matches!(old, JobState::Claimed(old) if old.expires_at().as_unix_micros() <= now))
                    && new.expires_at().as_unix_micros() > now
            }
            (JobOperation::Renew, JobState::Claimed(old), JobState::Claimed(new)) => {
                old.expires_at().as_unix_micros() > now.ok_or_else(malformed)?
                    && new.expires_at() > old.expires_at()
                    && new.tool() == old.tool()
                    && new.agent() == old.agent()
            }
            (JobOperation::Release, JobState::Claimed(old), JobState::Requested)
            | (JobOperation::Fail, JobState::Claimed(old), JobState::Failed(_))
            | (JobOperation::Complete, JobState::Claimed(old), JobState::Succeeded(_)) => {
                old.expires_at().as_unix_micros() > now.ok_or_else(malformed)?
            }
            (
                JobOperation::Cancel,
                JobState::Requested | JobState::Claimed(_),
                JobState::Cancelled,
            ) => true,
            _ => false,
        };
        if !valid {
            return Err(malformed());
        }
        Ok(Self {
            job,
            operation,
            previous,
            state,
            authority_time,
            input_boundary,
        })
    }

    /// Returns the observed job's stable identity.
    #[must_use]
    pub const fn job_id(&self) -> JobId {
        self.job
    }
    /// Returns the original semantic operation.
    #[must_use]
    pub const fn operation(&self) -> JobOperation {
        self.operation
    }
    /// Returns the state immediately before that operation.
    #[must_use]
    pub const fn previous(&self) -> &JobState {
        &self.previous
    }
    /// Returns the original resulting state.
    #[must_use]
    pub const fn state(&self) -> &JobState {
        &self.state
    }
    /// Returns the recorded authority time, without granting clock authority.
    #[must_use]
    pub const fn authority_time(&self) -> Option<Timestamp> {
        self.authority_time
    }
    /// Returns the original claim/completion input decision sequence.
    #[must_use]
    pub const fn input_boundary(&self) -> Option<u64> {
        self.input_boundary
    }

    /// Returns the original public observation implied by this operation.
    #[must_use]
    pub const fn observation(&self) -> RevisionEventKind {
        let job_id = self.job;
        match self.operation {
            JobOperation::Claim => RevisionEventKind::JobClaimed { job_id },
            JobOperation::Renew => RevisionEventKind::JobClaimRenewed { job_id },
            JobOperation::Release => RevisionEventKind::JobClaimReleased { job_id },
            JobOperation::Fail => RevisionEventKind::JobFailed { job_id },
            JobOperation::Complete => RevisionEventKind::JobSucceeded { job_id },
            JobOperation::Cancel => RevisionEventKind::JobCancelled { job_id },
        }
    }

    /// Encodes original scalar decisions and before/after observations.
    ///
    /// # Errors
    /// Rejects future observed states without a wire definition.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: json!({"kind":"job.transition", "job_id":self.job.to_string(),
            "operation":self.operation.wire_name(), "previous":state::encode(&self.previous)?,
            "state":state::encode(&self.state)?,
            "authority_time_micros":self.authority_time.map(|time|time.as_unix_micros().to_string()),
            "input_boundary":self.input_boundary.map(|sequence|sequence.to_string())}),
        })
    }

    /// Decodes through the same transition checks used for native capture.
    ///
    /// # Errors
    /// Rejects unknown fields, credentials and contradictory lifecycle detail.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "job_id",
                "operation",
                "previous",
                "state",
                "authority_time_micros",
                "input_boundary",
            ],
        )?;
        if document.kind()? != "job.transition" {
            return Err(unsupported());
        }
        Self::new(
            exact(&fields["job_id"])?,
            JobOperation::decode(text(&fields["operation"])?)?,
            state::decode(&fields["previous"])?,
            state::decode(&fields["state"])?,
            nullable(&fields["authority_time_micros"], |value| {
                Ok(Timestamp::from_unix_micros(exact(value)?))
            })?,
            nullable(&fields["input_boundary"], exact)?,
        )
    }
}
