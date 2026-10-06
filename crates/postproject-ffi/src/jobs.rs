//! C-ABI-owned projections of durable jobs.

use std::ffi::{CString, c_char};

use postproject_core::{
    Error, ErrorKind, Job, JobState, QueryCursor, RegenerationJobPlan, RepresentationKind,
};

use crate::{PpActivityId, PpAssetId, PpJobId, PpUuid, exact_cstring};

const PP_JOB_REQUESTED: u32 = 1;
pub(super) const PP_JOB_CLAIMED: u32 = 2;
pub(super) const PP_JOB_SUCCEEDED: u32 = 3;
pub(super) const PP_JOB_FAILED: u32 = 4;
const PP_JOB_CANCELLED: u32 = 5;

/// Opaque immutable job result set owned by the C caller.
pub struct PpJobSet {
    jobs: Vec<AbiJob>,
    next_cursor: Option<CString>,
}

/// Opaque immutable regeneration-plan result set owned by the C caller.
pub struct PpRegenerationPlanSet {
    plans: Vec<RegenerationJobPlan>,
}

/// Borrowed fixed-layout view of one durable job.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpJob {
    /// Stable job identity.
    pub id: PpJobId,
    /// Borrowed open-world job kind.
    pub kind: *const c_char,
    /// Asset that will own the requested output.
    pub output_asset_id: PpAssetId,
    /// Requested `PP_REPRESENTATION_*` kind.
    pub output_representation_kind: u32,
    /// Borrowed optional logical output-root name.
    pub target_root: *const c_char,
    /// One of the `PP_JOB_*` state constants.
    pub state: u32,
    /// Number of canonical input representations.
    pub input_count: u64,
    /// Active claim capability, or zero outside the claimed state.
    pub claim_id: PpUuid,
    /// Active lease expiry, or zero outside the claimed state.
    pub claim_expires_at_unix_micros: i64,
    /// Borrowed claiming tool fields, or null outside the claimed state.
    pub claim_tool_name: *const c_char,
    /// Borrowed optional claiming tool version.
    pub claim_tool_version: *const c_char,
    /// Borrowed optional claiming tool URI.
    pub claim_tool_uri: *const c_char,
    /// Borrowed optional claiming-agent fields.
    pub claim_agent_name: *const c_char,
    /// Borrowed optional claiming-agent identifier scheme.
    pub claim_agent_identifier_scheme: *const c_char,
    /// Borrowed optional claiming-agent identifier value.
    pub claim_agent_identifier_value: *const c_char,
    /// Borrowed optional claiming-agent identifier qualifier.
    pub claim_agent_identifier_qualifier: *const c_char,
    /// Completion facts, or zero outside the succeeded state.
    pub completion_activity_id: PpActivityId,
    /// Output representation, or zero outside the succeeded state.
    pub completion_representation_id: PpUuid,
    /// Borrowed diagnostic, or null outside the failed state.
    pub failure_diagnostic: *const c_char,
}

impl PpJob {
    pub(crate) const fn empty() -> Self {
        let zero = PpUuid { bytes: [0; 16] };
        Self {
            id: PpJobId { bytes: [0; 16] },
            kind: std::ptr::null(),
            output_asset_id: PpAssetId { bytes: [0; 16] },
            output_representation_kind: 0,
            target_root: std::ptr::null(),
            state: 0,
            input_count: 0,
            claim_id: zero,
            claim_expires_at_unix_micros: 0,
            claim_tool_name: std::ptr::null(),
            claim_tool_version: std::ptr::null(),
            claim_tool_uri: std::ptr::null(),
            claim_agent_name: std::ptr::null(),
            claim_agent_identifier_scheme: std::ptr::null(),
            claim_agent_identifier_value: std::ptr::null(),
            claim_agent_identifier_qualifier: std::ptr::null(),
            completion_activity_id: PpActivityId { bytes: [0; 16] },
            completion_representation_id: zero,
            failure_diagnostic: std::ptr::null(),
        }
    }
}

/// Borrowed detail returned only for a claimed job.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpJobClaim {
    /// Claim identity; capability migration remains separate.
    pub id: PpUuid,
    /// Current stored lease expiry.
    pub expires_at_unix_micros: i64,
    /// Required claiming tool name.
    pub tool_name: *const c_char,
    /// Optional tool version.
    pub tool_version: *const c_char,
    /// Optional tool URI.
    pub tool_uri: *const c_char,
    /// Optional agent name.
    pub agent_name: *const c_char,
    /// Optional agent identifier scheme.
    pub agent_identifier_scheme: *const c_char,
    /// Optional agent identifier value.
    pub agent_identifier_value: *const c_char,
    /// Optional agent identifier qualifier.
    pub agent_identifier_qualifier: *const c_char,
}

impl PpJobClaim {
    pub(crate) const fn from_job(job: &PpJob) -> Self {
        Self {
            id: job.claim_id,
            expires_at_unix_micros: job.claim_expires_at_unix_micros,
            tool_name: job.claim_tool_name,
            tool_version: job.claim_tool_version,
            tool_uri: job.claim_tool_uri,
            agent_name: job.claim_agent_name,
            agent_identifier_scheme: job.claim_agent_identifier_scheme,
            agent_identifier_value: job.claim_agent_identifier_value,
            agent_identifier_qualifier: job.claim_agent_identifier_qualifier,
        }
    }

    pub(crate) const fn empty() -> Self {
        Self::from_job(&PpJob::empty())
    }
}

/// Completion detail returned only for a succeeded job.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpJobCompletion {
    /// Completing activity.
    pub activity_id: PpActivityId,
    /// Produced representation.
    pub representation_id: PpUuid,
}

impl PpJobCompletion {
    pub(crate) const fn empty() -> Self {
        Self {
            activity_id: PpActivityId { bytes: [0; 16] },
            representation_id: PpUuid { bytes: [0; 16] },
        }
    }
}

struct AbiJob {
    value: PpJob,
    kind: CString,
    target_root: Option<CString>,
    claim_tool_name: Option<CString>,
    claim_tool_version: Option<CString>,
    claim_tool_uri: Option<CString>,
    claim_agent_name: Option<CString>,
    claim_agent_identifier_scheme: Option<CString>,
    claim_agent_identifier_value: Option<CString>,
    claim_agent_identifier_qualifier: Option<CString>,
    failure_diagnostic: Option<CString>,
    inputs: Vec<PpUuid>,
}

impl PpJobSet {
    pub(crate) fn new(jobs: &[Job]) -> Result<Self, Error> {
        Self::new_page(jobs, None)
    }

    pub(crate) fn new_page(jobs: &[Job], next_cursor: Option<&QueryCursor>) -> Result<Self, Error> {
        let jobs = jobs
            .iter()
            .map(AbiJob::try_from)
            .collect::<Result<_, _>>()?;
        let next_cursor = next_cursor
            .map(|cursor| exact_cstring(cursor.as_str(), "query cursor"))
            .transpose()?;
        Ok(Self { jobs, next_cursor })
    }

    pub(crate) fn len(&self) -> usize {
        self.jobs.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<PpJob> {
        self.jobs.get(index).map(AbiJob::as_abi)
    }

    pub(crate) fn get_for_state(&self, index: usize, state: u32) -> Result<PpJob, Error> {
        let job = self
            .get(index)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "job index is out of range"))?;
        if job.state != state {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "job status does not have the requested detail",
            ));
        }
        Ok(job)
    }

    pub(crate) fn input(&self, job_index: usize, input_index: usize) -> Option<PpUuid> {
        self.jobs.get(job_index)?.inputs.get(input_index).copied()
    }

    pub(crate) fn next_cursor(&self) -> *const c_char {
        self.next_cursor
            .as_ref()
            .map_or(std::ptr::null(), |cursor| cursor.as_ptr())
    }
}

impl PpRegenerationPlanSet {
    pub(crate) const fn new(plans: Vec<RegenerationJobPlan>) -> Self {
        Self { plans }
    }

    pub(crate) fn len(&self) -> usize {
        self.plans.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<&RegenerationJobPlan> {
        self.plans.get(index)
    }
}

impl AbiJob {
    fn as_abi(&self) -> PpJob {
        let mut value = self.value;
        value.kind = self.kind.as_ptr();
        value.target_root = optional_cstring_ptr(self.target_root.as_ref());
        value.claim_tool_name = optional_cstring_ptr(self.claim_tool_name.as_ref());
        value.claim_tool_version = optional_cstring_ptr(self.claim_tool_version.as_ref());
        value.claim_tool_uri = optional_cstring_ptr(self.claim_tool_uri.as_ref());
        value.claim_agent_name = optional_cstring_ptr(self.claim_agent_name.as_ref());
        value.claim_agent_identifier_scheme =
            optional_cstring_ptr(self.claim_agent_identifier_scheme.as_ref());
        value.claim_agent_identifier_value =
            optional_cstring_ptr(self.claim_agent_identifier_value.as_ref());
        value.claim_agent_identifier_qualifier =
            optional_cstring_ptr(self.claim_agent_identifier_qualifier.as_ref());
        value.failure_diagnostic = optional_cstring_ptr(self.failure_diagnostic.as_ref());
        value
    }
}

impl TryFrom<&Job> for AbiJob {
    type Error = Error;

    fn try_from(job: &Job) -> Result<Self, Self::Error> {
        let kind = exact_cstring(job.kind().as_str(), "job kind")?;
        let target_root = job
            .requested_output()
            .target_root()
            .map(|value| exact_cstring(value, "job target root"))
            .transpose()?;
        let mut value = empty_job(job, kind.as_ptr(), target_root.as_ref());
        let mut claim_tool_name = None;
        let mut claim_tool_version = None;
        let mut claim_tool_uri = None;
        let mut claim_agent_name = None;
        let mut claim_agent_identifier_scheme = None;
        let mut claim_agent_identifier_value = None;
        let mut claim_agent_identifier_qualifier = None;
        let mut failure_diagnostic = None;
        match job.state() {
            JobState::Requested => value.state = PP_JOB_REQUESTED,
            JobState::Claimed(claim) => {
                value.state = PP_JOB_CLAIMED;
                value.claim_id = PpUuid {
                    bytes: claim.id().into_bytes(),
                };
                value.claim_expires_at_unix_micros = claim.expires_at().as_unix_micros();
                claim_tool_name = Some(exact_cstring(claim.tool().name(), "claim tool name")?);
                claim_tool_version = claim
                    .tool()
                    .version()
                    .map(|text| exact_cstring(text, "claim tool version"))
                    .transpose()?;
                claim_tool_uri = claim
                    .tool()
                    .uri()
                    .map(|text| exact_cstring(text, "claim tool URI"))
                    .transpose()?;
                if let Some(agent) = claim.agent() {
                    claim_agent_name = agent
                        .name()
                        .map(|text| exact_cstring(text, "claim agent name"))
                        .transpose()?;
                    if let Some(identifier) = agent.identifier() {
                        claim_agent_identifier_scheme = Some(exact_cstring(
                            identifier.scheme().as_str(),
                            "claim agent identifier scheme",
                        )?);
                        claim_agent_identifier_value = Some(exact_cstring(
                            identifier.value(),
                            "claim agent identifier value",
                        )?);
                        claim_agent_identifier_qualifier = identifier
                            .qualifier()
                            .map(|text| exact_cstring(text, "claim agent identifier qualifier"))
                            .transpose()?;
                    }
                }
            }
            JobState::Succeeded(completion) => {
                value.state = PP_JOB_SUCCEEDED;
                value.completion_activity_id = PpActivityId {
                    bytes: completion.activity_id().into_bytes(),
                };
                value.completion_representation_id = PpUuid {
                    bytes: completion.representation_id().into_bytes(),
                };
            }
            JobState::Failed(failure) => {
                value.state = PP_JOB_FAILED;
                failure_diagnostic = Some(exact_cstring(
                    failure.diagnostic(),
                    "job failure diagnostic",
                )?);
            }
            JobState::Cancelled => value.state = PP_JOB_CANCELLED,
            _ => return Err(Error::new(ErrorKind::Unsupported, "unsupported job state")),
        }
        Ok(Self {
            value,
            kind,
            target_root,
            claim_tool_name,
            claim_tool_version,
            claim_tool_uri,
            claim_agent_name,
            claim_agent_identifier_scheme,
            claim_agent_identifier_value,
            claim_agent_identifier_qualifier,
            failure_diagnostic,
            inputs: job
                .inputs()
                .iter()
                .map(|id| PpUuid {
                    bytes: id.into_bytes(),
                })
                .collect(),
        })
    }
}

fn empty_job(job: &Job, kind: *const c_char, target_root: Option<&CString>) -> PpJob {
    PpJob {
        id: PpJobId {
            bytes: job.id().into_bytes(),
        },
        kind,
        output_asset_id: PpAssetId {
            bytes: job.requested_output().asset_id().into_bytes(),
        },
        output_representation_kind: representation_kind(
            job.requested_output().representation_kind(),
        ),
        target_root: optional_cstring_ptr(target_root),
        state: 0,
        input_count: u64::try_from(job.inputs().len()).unwrap_or(u64::MAX),
        ..PpJob::empty()
    }
}

fn optional_cstring_ptr(value: Option<&CString>) -> *const c_char {
    value.map_or(std::ptr::null(), |text| text.as_ptr())
}

const fn representation_kind(kind: RepresentationKind) -> u32 {
    match kind {
        RepresentationKind::Original => 1,
        RepresentationKind::Proxy => 2,
        RepresentationKind::Optimized => 3,
        RepresentationKind::Derived => 4,
        _ => 0,
    }
}
