//! Persisted work requests without scheduling or execution policy.

use crate::{
    ActivityId, AgentIdentity, AssetId, Error, ErrorKind, JobId, MediaRoot, MetadataAssertion,
    RepresentationId, RepresentationKind, Result, Timestamp, ToolIdentity,
};

/// Maximum encoded length of a namespaced job-kind identifier.
pub const MAX_JOB_KIND_BYTES: usize = 128;
/// Maximum number of input representations on one job.
pub const MAX_JOB_INPUTS: usize = 100_000;
/// Maximum UTF-8 byte length of a failure diagnostic.
pub const MAX_JOB_DIAGNOSTIC_BYTES: usize = 4_096;
/// Maximum artifacts accepted by one regeneration-planning call.
pub const MAX_REGENERATION_PLANS: usize = 100_000;

/// An open-world, namespaced kind of requested work.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JobKind(String);

impl JobKind {
    /// Creates a kind such as `org.postproject:generate-proxy`.
    ///
    /// # Errors
    ///
    /// Returns an invalid-argument error when the value is not a bounded,
    /// namespaced ASCII identifier.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let valid_bytes = value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'));
        let valid_namespace = value
            .split_once(':')
            .is_some_and(|(namespace, local)| !namespace.is_empty() && !local.is_empty());
        if value.len() > MAX_JOB_KIND_BYTES || !valid_bytes || !valid_namespace {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!(
                    "job kind must be a namespaced identifier of at most {MAX_JOB_KIND_BYTES} ASCII bytes"
                ),
            ));
        }
        Ok(Self(value))
    }

    /// Returns the exact kind identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The representation a successful job is expected to add.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestedJobOutput {
    asset_id: AssetId,
    representation_kind: RepresentationKind,
    target_root: Option<String>,
}

impl RequestedJobOutput {
    /// Creates a requested output with an optional logical media-root name.
    ///
    /// # Errors
    ///
    /// Returns an invalid-argument error when `target_root` is not a portable
    /// logical root name.
    pub fn new(
        asset_id: AssetId,
        representation_kind: RepresentationKind,
        target_root: Option<String>,
    ) -> Result<Self> {
        if let Some(name) = target_root.as_deref() {
            MediaRoot::validate_name(name)?;
        }
        Ok(Self {
            asset_id,
            representation_kind,
            target_root,
        })
    }

    /// Returns the asset that will own the output representation.
    #[must_use]
    pub const fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// Returns the semantic representation kind to create.
    #[must_use]
    pub const fn representation_kind(&self) -> RepresentationKind {
        self.representation_kind
    }

    /// Returns the optional logical output-root name.
    #[must_use]
    pub fn target_root(&self) -> Option<&str> {
        self.target_root.as_deref()
    }
}

/// Attribution and lease data for one active claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobClaim {
    tool: ToolIdentity,
    agent: Option<AgentIdentity>,
    expires_at: Timestamp,
}

impl JobClaim {
    /// Creates observed attribution and authority-selected expiry data.
    /// This value grants no worker ownership.
    #[must_use]
    pub const fn new(
        tool: ToolIdentity,
        agent: Option<AgentIdentity>,
        expires_at: Timestamp,
    ) -> Self {
        Self {
            tool,
            agent,
            expires_at,
        }
    }

    /// Returns the claiming tool identity.
    #[must_use]
    pub const fn tool(&self) -> &ToolIdentity {
        &self.tool
    }

    /// Returns optional agent attribution for the worker.
    #[must_use]
    pub const fn agent(&self) -> Option<&AgentIdentity> {
        self.agent.as_ref()
    }

    /// Returns the authority-selected lease expiry.
    #[must_use]
    pub const fn expires_at(&self) -> Timestamp {
        self.expires_at
    }
}

/// Durable result of successfully completing a job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobCompletion {
    activity_id: ActivityId,
    representation_id: RepresentationId,
}

impl JobCompletion {
    /// Creates a link to the atomically committed activity and output.
    #[must_use]
    pub const fn new(activity_id: ActivityId, representation_id: RepresentationId) -> Self {
        Self {
            activity_id,
            representation_id,
        }
    }

    /// Returns the activity that completed the requested work.
    #[must_use]
    pub const fn activity_id(self) -> ActivityId {
        self.activity_id
    }

    /// Returns the output representation produced by the activity.
    #[must_use]
    pub const fn representation_id(self) -> RepresentationId {
        self.representation_id
    }
}

/// Bounded diagnostic recorded for failed work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobFailure {
    diagnostic: String,
}

impl JobFailure {
    /// Creates a non-empty, bounded diagnostic.
    ///
    /// # Errors
    ///
    /// Returns an invalid-argument error for empty, oversized, or NUL-bearing
    /// text.
    pub fn new(diagnostic: impl Into<String>) -> Result<Self> {
        let diagnostic = diagnostic.into();
        if diagnostic.is_empty()
            || diagnostic.len() > MAX_JOB_DIAGNOSTIC_BYTES
            || diagnostic.contains('\0')
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!(
                    "job failure diagnostic must contain 1-{MAX_JOB_DIAGNOSTIC_BYTES} UTF-8 bytes without NUL"
                ),
            ));
        }
        Ok(Self { diagnostic })
    }

    /// Returns the exact recorded diagnostic.
    #[must_use]
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

/// Lifecycle state category for filtering and foreign interfaces.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum JobStateKind {
    /// Waiting for a worker.
    Requested,
    /// Held by a worker until its lease expires.
    Claimed,
    /// Completed with an activity and representation.
    Succeeded,
    /// Ended with a diagnostic and no output.
    Failed,
    /// Explicitly cancelled before completion.
    Cancelled,
}

/// Optional exact predicates for durable job enumeration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JobQuery {
    state: Option<JobStateKind>,
    kind: Option<JobKind>,
}

impl JobQuery {
    /// Creates a query from optional state and open-world kind predicates.
    #[must_use]
    pub const fn new(state: Option<JobStateKind>, kind: Option<JobKind>) -> Self {
        Self { state, kind }
    }

    /// Returns the exact state predicate, when present.
    #[must_use]
    pub const fn state(&self) -> Option<JobStateKind> {
        self.state
    }

    /// Returns the exact job-kind predicate, when present.
    #[must_use]
    pub const fn kind(&self) -> Option<&JobKind> {
        self.kind.as_ref()
    }
}

/// Validated lifecycle state and its state-specific detail.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum JobState {
    /// Waiting for a worker.
    Requested,
    /// Held by a worker until its lease expires.
    Claimed(JobClaim),
    /// Completed with an activity and output representation.
    Succeeded(JobCompletion),
    /// Ended with a bounded diagnostic.
    Failed(JobFailure),
    /// Explicitly cancelled.
    Cancelled,
}

impl JobState {
    /// Returns the state category without its detail.
    #[must_use]
    pub const fn kind(&self) -> JobStateKind {
        match self {
            Self::Requested => JobStateKind::Requested,
            Self::Claimed(_) => JobStateKind::Claimed,
            Self::Succeeded(_) => JobStateKind::Succeeded,
            Self::Failed(_) => JobStateKind::Failed,
            Self::Cancelled => JobStateKind::Cancelled,
        }
    }
}

/// One persisted request for production work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Job {
    id: JobId,
    kind: JobKind,
    inputs: Vec<RepresentationId>,
    requested_output: RequestedJobOutput,
    state: JobState,
}

/// A non-persisted job request and copied parameters for one artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegenerationJobPlan {
    artifact_representation_id: RepresentationId,
    job: Job,
    parameters: Vec<MetadataAssertion>,
}

impl RegenerationJobPlan {
    /// Creates a backend-derived regeneration plan without enqueuing it.
    #[doc(hidden)]
    #[must_use]
    pub fn new(
        artifact_representation_id: RepresentationId,
        job: Job,
        parameters: Vec<MetadataAssertion>,
    ) -> Self {
        Self {
            artifact_representation_id,
            job,
            parameters,
        }
    }

    /// Returns the existing artifact this request would regenerate.
    #[must_use]
    pub const fn artifact_representation_id(&self) -> RepresentationId {
        self.artifact_representation_id
    }

    /// Returns the requested job, which has not been persisted.
    #[must_use]
    pub const fn job(&self) -> &Job {
        &self.job
    }

    /// Returns activity parameter assertions copied for the future job target.
    #[must_use]
    pub fn parameters(&self) -> &[MetadataAssertion] {
        &self.parameters
    }
}

impl Job {
    /// Creates a requested job with canonical, distinct inputs.
    ///
    /// # Errors
    ///
    /// Returns an invalid-argument error for duplicate or excessive inputs.
    pub fn new(
        id: JobId,
        kind: JobKind,
        mut inputs: Vec<RepresentationId>,
        requested_output: RequestedJobOutput,
    ) -> Result<Self> {
        if inputs.len() > MAX_JOB_INPUTS {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("job must have at most {MAX_JOB_INPUTS} inputs"),
            ));
        }
        let supplied_count = inputs.len();
        inputs.sort_unstable();
        inputs.dedup();
        if inputs.len() != supplied_count {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "job input representations must be distinct",
            ));
        }
        Ok(Self {
            id,
            kind,
            inputs,
            requested_output,
            state: JobState::Requested,
        })
    }

    /// Returns the stable job identity.
    #[must_use]
    pub const fn id(&self) -> JobId {
        self.id
    }

    /// Returns the exact open-world job kind.
    #[must_use]
    pub const fn kind(&self) -> &JobKind {
        &self.kind
    }

    /// Returns canonical input representation identities.
    #[must_use]
    pub fn inputs(&self) -> &[RepresentationId] {
        &self.inputs
    }

    /// Returns the requested output description.
    #[must_use]
    pub const fn requested_output(&self) -> &RequestedJobOutput {
        &self.requested_output
    }

    /// Returns the lifecycle state and its state-specific detail.
    #[must_use]
    pub const fn state(&self) -> &JobState {
        &self.state
    }

    /// Replaces lifecycle state when reconstructing a storage-owned job.
    #[doc(hidden)]
    #[must_use]
    pub fn with_state(mut self, state: JobState) -> Self {
        self.state = state;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output() -> RequestedJobOutput {
        RequestedJobOutput::new(
            AssetId::from_bytes([1; 16]),
            RepresentationKind::Proxy,
            Some("proxies".to_owned()),
        )
        .expect("valid output")
    }

    #[test]
    fn job_kind_is_open_world_namespaced_text() {
        assert!(JobKind::new("org.postproject:generate-proxy").is_ok());
        assert!(JobKind::new("studio.example:custom_work").is_ok());
        assert!(JobKind::new("not-namespaced").is_err());
    }

    #[test]
    fn requested_jobs_canonicalize_and_reject_duplicate_inputs() {
        let first = RepresentationId::from_bytes([1; 16]);
        let second = RepresentationId::from_bytes([2; 16]);
        let job = Job::new(
            JobId::from_bytes([3; 16]),
            JobKind::new("org.postproject:render").expect("valid kind"),
            vec![second, first],
            output(),
        )
        .expect("valid job");
        assert_eq!(job.inputs(), [first, second]);
        assert_eq!(job.state().kind(), JobStateKind::Requested);
        assert!(
            Job::new(
                JobId::new(),
                JobKind::new("org.postproject:render").expect("valid kind"),
                vec![first, first],
                output(),
            )
            .is_err()
        );
    }

    #[test]
    fn diagnostics_and_output_roots_are_bounded() {
        assert!(JobFailure::new("").is_err());
        assert!(JobFailure::new("failed to encode").is_ok());
        assert!(
            RequestedJobOutput::new(
                AssetId::new(),
                RepresentationKind::Proxy,
                Some("../machine-path".to_owned()),
            )
            .is_err()
        );
    }
}
