//! C-ABI-owned projections of artifact knowledge and reproducibility.

use std::{ffi::CString, ptr};

use postproject_core::{
    ActivityId, ArtifactDependencyIssue, ArtifactDependencyPathSegment, ArtifactEdgeKind,
    ArtifactEvaluation, ArtifactKnowledgeReason, ArtifactKnowledgeState,
    ArtifactReproducibilityIssue, ArtifactReproducibilityReport, ArtifactTraversalLimitKind, Error,
    ErrorKind, RepresentationId,
};

use crate::{
    PP_OBJECT_ASSET, PP_OBJECT_REPRESENTATION, PpActivityId, PpObjectRef, PpRepresentationId,
    PpResourceId, PpUuid, exact_cstring,
};

/// Opaque immutable artifact-evaluation result owned by the C caller.
pub struct PpArtifactEvaluation {
    pub(crate) representation_id: RepresentationId,
    pub(crate) state: u32,
    pub(crate) visited_representations: u32,
    pub(crate) truncated: bool,
    pub(crate) reasons: Vec<AbiArtifactReason>,
}

/// Fixed-layout borrowed explanation for one artifact knowledge result.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpArtifactReason {
    /// One of the `PP_ARTIFACT_REASON_*` constants.
    pub kind: u32,
    /// Related activity, or zero when not applicable.
    pub activity_id: PpActivityId,
    /// Related representation, or zero when not applicable.
    pub representation_id: PpRepresentationId,
    /// Direct activity input, or zero when not applicable.
    pub input_representation_id: PpRepresentationId,
    /// Input/output edge kind, or zero when not applicable.
    pub edge_kind: u32,
    /// Upstream knowledge state, or zero when not applicable.
    pub upstream_state: u32,
    /// Traversal limit kind, or zero when not applicable.
    pub traversal_limit: u32,
    /// Ambiguous producer count, or zero when not applicable.
    pub activity_count: u32,
    /// One of the `PP_ARTIFACT_DEPENDENCY_*` constants, or zero.
    pub dependency_issue: u32,
    /// Borrowed typed dependency path, or null when empty.
    pub dependency_path: *const PpArtifactDependencyPathSegment,
    /// Number of path segments.
    pub dependency_path_length: u64,
    /// Borrowed fingerprint algorithm, or null when not applicable.
    pub fingerprint_algorithm: *const std::ffi::c_char,
    /// Fingerprint algorithm version, or zero when not applicable.
    pub fingerprint_version: u16,
    /// Whether the snapshot value is present.
    pub has_snapshot_value: u8,
    /// Borrowed snapshot bytes, or null when absent.
    pub snapshot_value: *const u8,
    /// Snapshot byte count.
    pub snapshot_value_length: u64,
    /// Whether the current value is present.
    pub has_current_value: u8,
    /// Borrowed current bytes, or null when absent.
    pub current_value: *const u8,
    /// Current byte count.
    pub current_value_length: u64,
}

/// Fixed-layout borrowed segment in an artifact dependency path.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpArtifactDependencyPathSegment {
    /// Representation whose content authored this dependency.
    pub source_representation_id: PpRepresentationId,
    /// Position in the source's complete dependency observation.
    pub dependency_position: u32,
    /// Whether `source_resource_id` is present.
    pub has_source_resource: u8,
    /// Optional resource containing the authored reference.
    pub source_resource_id: PpResourceId,
    /// Borrowed namespaced dependency kind.
    pub kind: *const std::ffi::c_char,
    /// Floating asset or pinned representation target.
    pub target: PpObjectRef,
    /// Whether `resolved_representation_id` is present.
    pub has_resolved_representation: u8,
    /// Representation selected for a floating target.
    pub resolved_representation_id: PpRepresentationId,
    /// Borrowed exact authored reference.
    pub authored_reference: *const std::ffi::c_char,
}

pub(crate) struct AbiArtifactReason {
    kind: u32,
    activity_id: Option<ActivityId>,
    representation_id: RepresentationId,
    input_representation_id: RepresentationId,
    edge_kind: u32,
    upstream_state: u32,
    traversal_limit: u32,
    activity_count: u32,
    dependency_issue: u32,
    dependency_path_storage: Vec<AbiArtifactDependencyPathSegment>,
    dependency_path: Vec<PpArtifactDependencyPathSegment>,
    fingerprint_algorithm: Option<CString>,
    fingerprint_version: u16,
    snapshot_value: Option<Vec<u8>>,
    current_value: Option<Vec<u8>>,
}

struct AbiArtifactDependencyPathSegment {
    source_representation_id: PpRepresentationId,
    dependency_position: u32,
    source_resource_id: Option<PpResourceId>,
    kind: CString,
    target: PpObjectRef,
    resolved_representation_id: Option<PpRepresentationId>,
    authored_reference: CString,
}

/// Opaque immutable artifact-reproducibility result owned by the C caller.
pub struct PpArtifactReproducibility {
    pub(crate) representation_id: RepresentationId,
    pub(crate) producing_activity_id: Option<ActivityId>,
    pub(crate) activity_kind: Option<CString>,
    pub(crate) issues: Vec<PpArtifactReproducibilityIssue>,
}

/// Fixed-layout reproducibility issue.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpArtifactReproducibilityIssue {
    /// One of the `PP_ARTIFACT_REPRODUCIBILITY_*` constants.
    pub kind: u32,
    /// Related activity, or zero when not applicable.
    pub activity_id: PpActivityId,
    /// Missing input representation, or zero when not applicable.
    pub representation_id: PpRepresentationId,
    /// Ambiguous producer count, or zero when not applicable.
    pub activity_count: u32,
}

impl PpArtifactEvaluation {
    pub(crate) fn new(evaluation: &ArtifactEvaluation) -> Result<Self, Error> {
        Ok(Self {
            representation_id: evaluation.representation_id(),
            state: knowledge_state(evaluation.state()),
            visited_representations: evaluation.visited_representations(),
            truncated: evaluation.is_truncated(),
            reasons: evaluation
                .reasons()
                .iter()
                .map(AbiArtifactReason::try_from)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl AbiArtifactReason {
    pub(crate) fn as_abi(&self) -> PpArtifactReason {
        PpArtifactReason {
            kind: self.kind,
            activity_id: PpActivityId {
                bytes: self.activity_id.map_or([0; 16], ActivityId::into_bytes),
            },
            representation_id: PpRepresentationId {
                bytes: self.representation_id.into_bytes(),
            },
            input_representation_id: PpRepresentationId {
                bytes: self.input_representation_id.into_bytes(),
            },
            edge_kind: self.edge_kind,
            upstream_state: self.upstream_state,
            traversal_limit: self.traversal_limit,
            activity_count: self.activity_count,
            dependency_issue: self.dependency_issue,
            dependency_path: self
                .dependency_path
                .first()
                .map_or(ptr::null(), std::ptr::from_ref),
            dependency_path_length: u64::try_from(self.dependency_path.len()).unwrap_or(u64::MAX),
            fingerprint_algorithm: self
                .fingerprint_algorithm
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
            fingerprint_version: self.fingerprint_version,
            has_snapshot_value: u8::from(self.snapshot_value.is_some()),
            snapshot_value: bytes_ptr(self.snapshot_value.as_deref()),
            snapshot_value_length: bytes_len(self.snapshot_value.as_deref()),
            has_current_value: u8::from(self.current_value.is_some()),
            current_value: bytes_ptr(self.current_value.as_deref()),
            current_value_length: bytes_len(self.current_value.as_deref()),
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the complete reason projection is clearest as one exhaustive mapping"
)]
impl TryFrom<&ArtifactKnowledgeReason> for AbiArtifactReason {
    type Error = Error;

    fn try_from(reason: &ArtifactKnowledgeReason) -> Result<Self, Self::Error> {
        let mut projected = Self {
            kind: 0,
            activity_id: None,
            representation_id: RepresentationId::from_bytes([0; 16]),
            input_representation_id: RepresentationId::from_bytes([0; 16]),
            edge_kind: 0,
            upstream_state: 0,
            traversal_limit: 0,
            activity_count: 0,
            dependency_issue: 0,
            dependency_path_storage: Vec::new(),
            dependency_path: Vec::new(),
            fingerprint_algorithm: None,
            fingerprint_version: 0,
            snapshot_value: None,
            current_value: None,
        };
        match reason {
            ArtifactKnowledgeReason::ProducingActivityMissing { representation_id } => {
                projected.kind = 1;
                projected.representation_id = *representation_id;
            }
            ArtifactKnowledgeReason::ProducingActivityAmbiguous {
                representation_id,
                activity_count,
            } => {
                projected.kind = 2;
                projected.representation_id = *representation_id;
                projected.activity_count = *activity_count;
            }
            ArtifactKnowledgeReason::SnapshotAbsent {
                activity_id,
                representation_id,
                edge,
            } => {
                projected.kind = 3;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *representation_id;
                projected.edge_kind = edge_kind(*edge);
            }
            ArtifactKnowledgeReason::FingerprintEvidenceMissing {
                activity_id,
                representation_id,
                edge,
                algorithm,
                version,
                snapshot_value,
                current_value,
            } => {
                projected.kind = 4;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *representation_id;
                projected.edge_kind = edge_kind(*edge);
                projected.fingerprint_algorithm = algorithm
                    .as_deref()
                    .map(|value| exact_cstring(value, "artifact fingerprint algorithm"))
                    .transpose()?;
                projected.fingerprint_version = version.unwrap_or(0);
                projected.snapshot_value.clone_from(snapshot_value);
                projected.current_value.clone_from(current_value);
            }
            ArtifactKnowledgeReason::FingerprintChanged {
                activity_id,
                representation_id,
                edge,
                algorithm,
                version,
                snapshot_value,
                current_value,
            } => {
                projected.kind = 5;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *representation_id;
                projected.edge_kind = edge_kind(*edge);
                projected.fingerprint_algorithm =
                    Some(exact_cstring(algorithm, "artifact fingerprint algorithm")?);
                projected.fingerprint_version = *version;
                projected.snapshot_value = Some(snapshot_value.clone());
                projected.current_value = Some(current_value.clone());
            }
            ArtifactKnowledgeReason::FingerprintRecomputationPending {
                activity_id,
                representation_id,
                edge,
            } => {
                projected.kind = 6;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *representation_id;
                projected.edge_kind = edge_kind(*edge);
            }
            ArtifactKnowledgeReason::UpstreamNotCurrent {
                representation_id,
                state,
            } => {
                projected.kind = 7;
                projected.representation_id = *representation_id;
                projected.upstream_state = knowledge_state(*state);
            }
            ArtifactKnowledgeReason::TraversalTruncated {
                limit,
                representation_id,
            } => {
                projected.kind = 8;
                projected.representation_id = *representation_id;
                projected.traversal_limit = match limit {
                    ArtifactTraversalLimitKind::Depth => 1,
                    ArtifactTraversalLimitKind::Representations => 2,
                    _ => 0,
                };
            }
            ArtifactKnowledgeReason::DependencySnapshotAbsent {
                activity_id,
                representation_id,
            } => {
                projected.kind = 9;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *representation_id;
                projected.input_representation_id = *representation_id;
            }
            ArtifactKnowledgeReason::DependencyKnowledgeIncomplete {
                activity_id,
                input_representation_id,
                subject_representation_id,
                path,
                issue,
            } => {
                projected.kind = 10;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *subject_representation_id;
                projected.input_representation_id = *input_representation_id;
                projected.dependency_issue = dependency_issue(*issue);
                projected.set_dependency_path(path)?;
            }
            ArtifactKnowledgeReason::DependencyPathChanged {
                activity_id,
                input_representation_id,
                path,
            } => {
                projected.kind = 11;
                projected.activity_id = Some(*activity_id);
                projected.input_representation_id = *input_representation_id;
                projected.set_dependency_path(path)?;
            }
            ArtifactKnowledgeReason::DependencyFingerprintChanged {
                activity_id,
                input_representation_id,
                representation_id,
                path,
                algorithm,
                version,
                snapshot_value,
                current_value,
            } => {
                projected.kind = 12;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *representation_id;
                projected.input_representation_id = *input_representation_id;
                projected.set_dependency_path(path)?;
                projected.fingerprint_algorithm =
                    Some(exact_cstring(algorithm, "artifact fingerprint algorithm")?);
                projected.fingerprint_version = *version;
                projected.snapshot_value = Some(snapshot_value.clone());
                projected.current_value = Some(current_value.clone());
            }
            ArtifactKnowledgeReason::DependencyFingerprintRecomputationPending {
                activity_id,
                input_representation_id,
                representation_id,
                path,
            } => {
                projected.kind = 13;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *representation_id;
                projected.input_representation_id = *input_representation_id;
                projected.set_dependency_path(path)?;
            }
            ArtifactKnowledgeReason::DependencyFingerprintEvidenceMissing {
                activity_id,
                input_representation_id,
                representation_id,
                path,
                algorithm,
                version,
                snapshot_value,
                current_value,
            } => {
                projected.kind = 14;
                projected.activity_id = Some(*activity_id);
                projected.representation_id = *representation_id;
                projected.input_representation_id = *input_representation_id;
                projected.set_dependency_path(path)?;
                projected.fingerprint_algorithm = algorithm
                    .as_deref()
                    .map(|value| exact_cstring(value, "artifact fingerprint algorithm"))
                    .transpose()?;
                projected.fingerprint_version = version.unwrap_or(0);
                projected.snapshot_value.clone_from(snapshot_value);
                projected.current_value.clone_from(current_value);
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "artifact reason is not supported by this ABI",
                ));
            }
        }
        Ok(projected)
    }
}

impl AbiArtifactReason {
    fn set_dependency_path(&mut self, path: &[ArtifactDependencyPathSegment]) -> Result<(), Error> {
        self.dependency_path_storage = path
            .iter()
            .map(AbiArtifactDependencyPathSegment::try_from)
            .collect::<Result<_, _>>()?;
        self.dependency_path = self
            .dependency_path_storage
            .iter()
            .map(AbiArtifactDependencyPathSegment::as_abi)
            .collect();
        Ok(())
    }
}

impl AbiArtifactDependencyPathSegment {
    fn as_abi(&self) -> PpArtifactDependencyPathSegment {
        PpArtifactDependencyPathSegment {
            source_representation_id: self.source_representation_id,
            dependency_position: self.dependency_position,
            has_source_resource: u8::from(self.source_resource_id.is_some()),
            source_resource_id: self
                .source_resource_id
                .unwrap_or(PpResourceId { bytes: [0; 16] }),
            kind: self.kind.as_ptr(),
            target: self.target,
            has_resolved_representation: u8::from(self.resolved_representation_id.is_some()),
            resolved_representation_id: self
                .resolved_representation_id
                .unwrap_or(PpRepresentationId { bytes: [0; 16] }),
            authored_reference: self.authored_reference.as_ptr(),
        }
    }
}

impl TryFrom<&ArtifactDependencyPathSegment> for AbiArtifactDependencyPathSegment {
    type Error = Error;

    fn try_from(segment: &ArtifactDependencyPathSegment) -> Result<Self, Self::Error> {
        let (target_kind, target_id) = match segment.target() {
            postproject_core::DependencyTarget::Asset(id) => (PP_OBJECT_ASSET, id.into_bytes()),
            postproject_core::DependencyTarget::Representation(id) => {
                (PP_OBJECT_REPRESENTATION, id.into_bytes())
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "dependency target is not supported by this ABI",
                ));
            }
        };
        Ok(Self {
            source_representation_id: PpRepresentationId {
                bytes: segment.source_representation_id().into_bytes(),
            },
            dependency_position: segment.dependency_position(),
            source_resource_id: segment.source_resource_id().map(|id| PpResourceId {
                bytes: id.into_bytes(),
            }),
            kind: exact_cstring(segment.kind().as_str(), "dependency kind")?,
            target: PpObjectRef {
                kind: target_kind,
                id: PpUuid { bytes: target_id },
            },
            resolved_representation_id: segment.resolved_representation_id().map(|id| {
                PpRepresentationId {
                    bytes: id.into_bytes(),
                }
            }),
            authored_reference: exact_cstring(
                segment.authored_reference(),
                "authored dependency reference",
            )?,
        })
    }
}

const fn dependency_issue(issue: ArtifactDependencyIssue) -> u32 {
    match issue {
        ArtifactDependencyIssue::NeedsExtraction => 1,
        ArtifactDependencyIssue::Unresolved => 2,
        ArtifactDependencyIssue::DepthTruncated => 3,
        ArtifactDependencyIssue::RepresentationsTruncated => 4,
        _ => 0,
    }
}

impl PpArtifactReproducibility {
    pub(crate) fn new(report: &ArtifactReproducibilityReport) -> Result<Self, Error> {
        Ok(Self {
            representation_id: report.representation_id(),
            producing_activity_id: report.producing_activity_id(),
            activity_kind: report
                .activity_kind()
                .map(|value| exact_cstring(value.as_str(), "artifact activity kind"))
                .transpose()?,
            issues: report
                .issues()
                .iter()
                .map(reproducibility_issue)
                .collect::<Result<_, _>>()?,
        })
    }
}

fn reproducibility_issue(
    issue: &ArtifactReproducibilityIssue,
) -> Result<PpArtifactReproducibilityIssue, Error> {
    let mut projected = PpArtifactReproducibilityIssue {
        kind: 0,
        activity_id: PpActivityId { bytes: [0; 16] },
        representation_id: PpRepresentationId { bytes: [0; 16] },
        activity_count: 0,
    };
    match issue {
        ArtifactReproducibilityIssue::ProducingActivityMissing => projected.kind = 1,
        ArtifactReproducibilityIssue::ProducingActivityAmbiguous { activity_count } => {
            projected.kind = 2;
            projected.activity_count = *activity_count;
        }
        ArtifactReproducibilityIssue::ToolIdentityMissing { activity_id } => {
            projected.kind = 3;
            projected.activity_id.bytes = activity_id.into_bytes();
        }
        ArtifactReproducibilityIssue::ParametersMissing { activity_id } => {
            projected.kind = 4;
            projected.activity_id.bytes = activity_id.into_bytes();
        }
        ArtifactReproducibilityIssue::InputRepresentationMissing {
            activity_id,
            representation_id,
        } => {
            projected.kind = 5;
            projected.activity_id.bytes = activity_id.into_bytes();
            projected.representation_id.bytes = representation_id.into_bytes();
        }
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "artifact reproducibility issue is not supported by this ABI",
            ));
        }
    }
    Ok(projected)
}

pub(crate) const fn knowledge_state(state: ArtifactKnowledgeState) -> u32 {
    match state {
        ArtifactKnowledgeState::Current => 1,
        ArtifactKnowledgeState::Stale => 2,
        ArtifactKnowledgeState::Indeterminate => 3,
        ArtifactKnowledgeState::Diverged => 4,
        _ => 0,
    }
}

const fn edge_kind(edge: ArtifactEdgeKind) -> u32 {
    match edge {
        ArtifactEdgeKind::Input => 1,
        ArtifactEdgeKind::Output => 2,
        _ => 0,
    }
}

fn bytes_ptr(value: Option<&[u8]>) -> *const u8 {
    value.map_or(ptr::null(), <[u8]>::as_ptr)
}

fn bytes_len(value: Option<&[u8]>) -> u64 {
    value.map_or(0, |bytes| u64::try_from(bytes.len()).unwrap_or(u64::MAX))
}
