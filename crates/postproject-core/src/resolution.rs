//! Explainable and deterministic media-resolution results.

use std::cmp::Reverse;

use crate::{
    ContentStructure, Error, ErrorKind, MAX_SEQUENCE_EXCEPTIONS, MediaRoot, RepresentationId,
    ResourceId, Result, SequenceNaming, uri::normalize_uri,
};

/// A deterministic confidence value in basis points from 0 through 10,000.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Confidence(u16);

impl Confidence {
    /// Certain confidence used only for verified identity evidence.
    pub const CERTAIN: Self = Self(10_000);

    /// Creates a confidence value, rejecting values above 100%.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when `value` exceeds 10,000.
    pub fn from_basis_points(value: u16) -> Result<Self> {
        if value > 10_000 {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "confidence must not exceed 10,000 basis points",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the confidence in basis points.
    #[must_use]
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

/// A machine-inspectable reason supporting or opposing a resolution candidate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum EvidenceKind {
    /// A persisted locator is currently available.
    KnownLocatorAvailable,
    /// The complete stored fingerprint matches.
    ExactFingerprintMatch,
    /// A cryptographic full-file digest matches.
    FullHashMatch,
    /// A sampled or otherwise partial fingerprint matches.
    PartialFingerprintMatch,
    /// The byte size matches.
    FileSizeMatch,
    /// The final path component matches.
    FileNameMatch,
    /// The path relative to a media root is similar.
    RelativePathSimilarity,
    /// The candidate is contained in a configured media root.
    MediaRootRelation,
    /// A logical media root has no mapping on this machine.
    MediaRootUnmapped,
    /// A mapped media root cannot currently be searched.
    MediaRootUnavailable,
    /// Present content does not match its stored fingerprint evidence.
    FingerprintMismatch,
    /// Stored fingerprint evidence exists only in domains the resolver cannot
    /// compute, so content identity was not checked.
    FingerprintNotVerified,
    /// A searched directory held more entries than the per-directory budget,
    /// so part of it was not searched.
    SearchTruncated,
    /// Another candidate has equivalent credible evidence.
    ConflictingCandidate,
    /// Candidate discovery or verification could not complete safely.
    DiscoveryError,
}

/// Structured evidence with optional human-readable context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionEvidence {
    kind: EvidenceKind,
    detail: Option<String>,
}

impl ResolutionEvidence {
    /// Creates a structured evidence item.
    #[must_use]
    pub fn new(kind: EvidenceKind, detail: Option<String>) -> Self {
        Self { kind, detail }
    }

    /// Returns the machine-readable evidence kind.
    #[must_use]
    pub const fn kind(&self) -> EvidenceKind {
        self.kind
    }

    /// Returns optional diagnostic context.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

/// A possible resource locator considered by the resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionCandidate {
    uri: String,
    confidence: Confidence,
    evidence: Vec<ResolutionEvidence>,
    media_root: Option<String>,
    sequence_naming: Option<SequenceNaming>,
}

impl ResolutionCandidate {
    /// Creates a candidate with an absolute URI and at least one evidence item.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when `uri` is invalid or relative,
    /// or when `evidence` is empty.
    pub fn new(
        uri: impl Into<String>,
        confidence: Confidence,
        evidence: Vec<ResolutionEvidence>,
    ) -> Result<Self> {
        let uri = normalize_uri(uri, "resolution candidate")?;
        if evidence.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "resolution candidate must contain evidence",
            ));
        }
        Ok(Self {
            uri,
            confidence,
            evidence,
            media_root: None,
            sequence_naming: None,
        })
    }

    /// Records the naming of the image-sequence files the candidate was found
    /// under, so confirmation can record it with the locator.
    #[must_use]
    pub fn with_sequence_naming(mut self, naming: SequenceNaming) -> Self {
        self.sequence_naming = Some(naming);
        self
    }

    /// Returns the naming of the sequence files at the candidate, present
    /// exactly for a candidate of an image-sequence resource.
    #[must_use]
    pub const fn sequence_naming(&self) -> Option<&SequenceNaming> {
        self.sequence_naming.as_ref()
    }

    /// Associates the candidate with the logical media root it was found
    /// under, so confirmation can record the portable root name.
    ///
    /// # Errors
    ///
    /// Returns an invalid-argument error for an invalid logical root name.
    pub fn with_media_root(mut self, name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        MediaRoot::validate_name(&name)?;
        self.media_root = Some(name);
        Ok(self)
    }

    /// Returns the logical media root the candidate was found under, if any.
    /// A candidate found in an unnamed search directory has none.
    #[must_use]
    pub fn media_root(&self) -> Option<&str> {
        self.media_root.as_deref()
    }

    /// Returns the candidate URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the deterministic confidence value.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Returns the inspectable supporting evidence.
    #[must_use]
    pub fn evidence(&self) -> &[ResolutionEvidence] {
        &self.evidence
    }
}

/// The outcome of resolving one storage resource.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ResourceResolutionState {
    /// A persisted locator is currently online.
    OnlineAtKnownLocator,
    /// One candidate has exact identity evidence.
    ResolvedExact,
    /// One candidate is credible but lacks exact verification.
    ResolvedProbable,
    /// No credible candidate was found.
    Offline,
    /// Multiple candidates require an explicit decision.
    Ambiguous,
    /// Candidate discovery or verification could not complete safely.
    Error,
}

/// An explainable result for one resource, independent of representation shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceResolution {
    resource_id: ResourceId,
    state: ResourceResolutionState,
    candidates: Vec<ResolutionCandidate>,
    evidence: Vec<ResolutionEvidence>,
    missing_frames: Vec<i64>,
}

impl ResourceResolution {
    /// Creates a resource result and sorts candidates deterministically.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the candidate count is
    /// inconsistent with `state`.
    pub fn new(
        resource_id: ResourceId,
        state: ResourceResolutionState,
        mut candidates: Vec<ResolutionCandidate>,
        evidence: Vec<ResolutionEvidence>,
    ) -> Result<Self> {
        candidates.sort_by(|left, right| {
            (
                Reverse(left.confidence),
                left.uri.as_str(),
                &left.sequence_naming,
            )
                .cmp(&(
                    Reverse(right.confidence),
                    right.uri.as_str(),
                    &right.sequence_naming,
                ))
        });
        let valid_count = match state {
            ResourceResolutionState::OnlineAtKnownLocator
            | ResourceResolutionState::ResolvedExact
            | ResourceResolutionState::ResolvedProbable => candidates.len() == 1,
            ResourceResolutionState::Offline | ResourceResolutionState::Error => {
                candidates.is_empty()
            }
            ResourceResolutionState::Ambiguous => candidates.len() >= 2,
        };
        if !valid_count {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "candidate count is inconsistent with resource resolution state",
            ));
        }
        Ok(Self {
            resource_id,
            state,
            candidates,
            evidence,
            missing_frames: Vec::new(),
        })
    }

    /// Adds observed missing image-sequence frames in canonical order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the diagnostic exceeds the
    /// bounded sequence-exception limit.
    pub fn with_missing_frames(mut self, mut missing_frames: Vec<i64>) -> Result<Self> {
        if missing_frames.len() > MAX_SEQUENCE_EXCEPTIONS {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("resolution has more than {MAX_SEQUENCE_EXCEPTIONS} missing frames"),
            ));
        }
        missing_frames.sort_unstable();
        missing_frames.dedup();
        self.missing_frames = missing_frames;
        Ok(self)
    }

    /// Returns the resolved resource identity.
    #[must_use]
    pub const fn resource_id(&self) -> ResourceId {
        self.resource_id
    }

    /// Returns the resource outcome.
    #[must_use]
    pub const fn state(&self) -> ResourceResolutionState {
        self.state
    }

    /// Returns candidates in deterministic best-first order.
    #[must_use]
    pub fn candidates(&self) -> &[ResolutionCandidate] {
        &self.candidates
    }

    /// Returns resource-wide evidence and diagnostics.
    #[must_use]
    pub fn evidence(&self) -> &[ResolutionEvidence] {
        &self.evidence
    }

    /// Returns sorted frames observed absent while resolving this resource.
    #[must_use]
    pub fn missing_frames(&self) -> &[i64] {
        &self.missing_frames
    }
}

/// Aggregated availability of a complete representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum RepresentationAvailability {
    /// Every required resource and frame is resolvable.
    Online,
    /// Some required content is resolvable and some is unavailable.
    Partial,
    /// No required content is resolvable.
    Offline,
    /// A required resource has multiple plausible candidates.
    Ambiguous,
    /// Required resource resolution could not complete safely.
    Error,
}

/// The machine-inspectable category of a representation availability issue.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum AvailabilityIssueKind {
    /// A resource has no credible online candidate.
    OfflineResource,
    /// A resource has multiple plausible candidates.
    AmbiguousResource,
    /// Resolution of a resource failed safely.
    ResourceError,
    /// Known frames are absent from an image sequence.
    MissingFrames,
}

/// A resource- or frame-specific availability diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvailabilityIssue {
    resource_id: ResourceId,
    required: bool,
    kind: AvailabilityIssueKind,
    frames: Vec<i64>,
}

impl AvailabilityIssue {
    /// Returns the affected resource or compact sequence resource.
    #[must_use]
    pub const fn resource_id(&self) -> ResourceId {
        self.resource_id
    }

    /// Returns whether this content is required for a complete representation.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }

    /// Returns the diagnostic category.
    #[must_use]
    pub const fn kind(&self) -> AvailabilityIssueKind {
        self.kind
    }

    /// Returns sorted missing frames for [`AvailabilityIssueKind::MissingFrames`].
    #[must_use]
    pub fn frames(&self) -> &[i64] {
        &self.frames
    }
}

/// Representation-level availability with ordered per-resource detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepresentationResolution {
    representation_id: RepresentationId,
    availability: RepresentationAvailability,
    resources: Vec<ResourceResolution>,
    issues: Vec<AvailabilityIssue>,
}

impl RepresentationResolution {
    /// Aggregates resource results according to content-structure requiredness.
    ///
    /// Optional package members produce diagnostics but do not reduce aggregate
    /// availability. Known missing sequence frames make an otherwise online
    /// sequence partial.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] unless the resource results match
    /// the structure's resources exactly and without duplicates.
    pub fn aggregate(
        representation_id: RepresentationId,
        structure: &ContentStructure,
        resources: Vec<ResourceResolution>,
    ) -> Result<Self> {
        let ordered = order_resource_resolutions(structure, resources)?;
        let (availability, issues) = inspect_availability(structure, &ordered);
        Ok(Self {
            representation_id,
            availability,
            resources: ordered,
            issues,
        })
    }

    /// Returns the representation whose availability was aggregated.
    #[must_use]
    pub const fn representation_id(&self) -> RepresentationId {
        self.representation_id
    }

    /// Returns aggregate representation availability.
    #[must_use]
    pub const fn availability(&self) -> RepresentationAvailability {
        self.availability
    }

    /// Returns results in content-structure resource order.
    #[must_use]
    pub fn resources(&self) -> &[ResourceResolution] {
        &self.resources
    }

    /// Returns resource- and frame-specific diagnostics.
    #[must_use]
    pub fn issues(&self) -> &[AvailabilityIssue] {
        &self.issues
    }
}

#[derive(Default)]
struct AvailabilityCounts {
    online_required: usize,
    offline_required: usize,
    ambiguous_required: bool,
    error_required: bool,
    missing_frames: bool,
}

impl AvailabilityCounts {
    const fn availability(&self) -> RepresentationAvailability {
        if self.error_required {
            RepresentationAvailability::Error
        } else if self.ambiguous_required {
            RepresentationAvailability::Ambiguous
        } else if self.online_required == 0 {
            RepresentationAvailability::Offline
        } else if self.offline_required > 0 || self.missing_frames {
            RepresentationAvailability::Partial
        } else {
            RepresentationAvailability::Online
        }
    }
}

fn order_resource_resolutions(
    structure: &ContentStructure,
    resources: Vec<ResourceResolution>,
) -> Result<Vec<ResourceResolution>> {
    let expected = structure.resource_ids();
    if resources.len() != expected.len() {
        return Err(resource_set_error());
    }
    let mut by_id = std::collections::BTreeMap::new();
    for resource in resources {
        if by_id.insert(resource.resource_id(), resource).is_some() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "resource resolution is duplicated",
            ));
        }
    }
    expected
        .into_iter()
        .map(|resource_id| by_id.remove(&resource_id).ok_or_else(resource_set_error))
        .collect()
}

fn resource_set_error() -> Error {
    Error::new(
        ErrorKind::InvalidArgument,
        "resource resolutions do not match the content structure",
    )
}

fn inspect_availability(
    structure: &ContentStructure,
    resources: &[ResourceResolution],
) -> (RepresentationAvailability, Vec<AvailabilityIssue>) {
    let mut counts = AvailabilityCounts::default();
    let mut issues = Vec::new();
    for resource in resources {
        inspect_resource(structure, resource, &mut counts, &mut issues);
    }
    if let Some(descriptor) = structure.image_sequence_descriptor() {
        let mut missing_frames = descriptor.known_missing_frames().to_vec();
        if let Some(resource) = resources
            .iter()
            .find(|resource| resource.resource_id() == descriptor.resource_id())
        {
            missing_frames.extend_from_slice(resource.missing_frames());
        }
        missing_frames.sort_unstable();
        missing_frames.dedup();
        if !missing_frames.is_empty() {
            counts.missing_frames = true;
            issues.push(AvailabilityIssue {
                resource_id: descriptor.resource_id(),
                required: true,
                kind: AvailabilityIssueKind::MissingFrames,
                frames: missing_frames,
            });
        }
    }
    (counts.availability(), issues)
}

fn inspect_resource(
    structure: &ContentStructure,
    resource: &ResourceResolution,
    counts: &mut AvailabilityCounts,
    issues: &mut Vec<AvailabilityIssue>,
) {
    let resource_id = resource.resource_id();
    let required = resource_is_required(structure, resource_id);
    let issue_kind = match resource.state() {
        ResourceResolutionState::OnlineAtKnownLocator
        | ResourceResolutionState::ResolvedExact
        | ResourceResolutionState::ResolvedProbable => {
            counts.online_required += usize::from(required);
            None
        }
        ResourceResolutionState::Offline => {
            counts.offline_required += usize::from(required);
            Some(AvailabilityIssueKind::OfflineResource)
        }
        ResourceResolutionState::Ambiguous => {
            counts.ambiguous_required |= required;
            Some(AvailabilityIssueKind::AmbiguousResource)
        }
        ResourceResolutionState::Error => {
            counts.error_required |= required;
            Some(AvailabilityIssueKind::ResourceError)
        }
    };
    if let Some(kind) = issue_kind {
        issues.push(AvailabilityIssue {
            resource_id,
            required,
            kind,
            frames: Vec::new(),
        });
    }
}

fn resource_is_required(structure: &ContentStructure, resource_id: ResourceId) -> bool {
    structure.members().is_none_or(|members| {
        members
            .iter()
            .find(|member| member.resource_id() == resource_id)
            .is_some_and(crate::ResourceMember::is_required)
    })
}

#[cfg(test)]
mod tests {
    use crate::{FrameRange, ImageSequenceDescriptor, RationalRate, ResourceMember, ResourceRole};
    use proptest::prelude::*;

    use super::*;

    fn candidate(uri: &str, confidence: u16) -> ResolutionCandidate {
        ResolutionCandidate::new(
            uri,
            Confidence::from_basis_points(confidence).expect("test confidence is valid"),
            vec![ResolutionEvidence::new(EvidenceKind::FileSizeMatch, None)],
        )
        .expect("test candidate is valid")
    }

    fn resource_result(
        resource_id: ResourceId,
        state: ResourceResolutionState,
    ) -> ResourceResolution {
        let candidates = match state {
            ResourceResolutionState::OnlineAtKnownLocator
            | ResourceResolutionState::ResolvedExact
            | ResourceResolutionState::ResolvedProbable => {
                vec![candidate(&format!("file:///{resource_id}"), 10_000)]
            }
            ResourceResolutionState::Ambiguous => vec![
                candidate(&format!("file:///a/{resource_id}"), 9_000),
                candidate(&format!("file:///b/{resource_id}"), 9_000),
            ],
            ResourceResolutionState::Offline | ResourceResolutionState::Error => Vec::new(),
        };
        ResourceResolution::new(resource_id, state, candidates, Vec::new())
            .expect("test resource resolution is valid")
    }

    #[test]
    fn candidates_are_sorted_deterministically() {
        let resolution = ResourceResolution::new(
            ResourceId::new(),
            ResourceResolutionState::Ambiguous,
            vec![
                candidate("file:///z", 8_000),
                candidate("file:///b", 9_000),
                candidate("file:///a", 9_000),
            ],
            Vec::new(),
        )
        .expect("ambiguous result has enough candidates");

        let uris: Vec<_> = resolution
            .candidates()
            .iter()
            .map(ResolutionCandidate::uri)
            .collect();
        assert_eq!(uris, ["file:///a", "file:///b", "file:///z"]);
    }

    #[test]
    fn ambiguity_cannot_silently_select_one_candidate() {
        let error = ResourceResolution::new(
            ResourceId::new(),
            ResourceResolutionState::Ambiguous,
            vec![candidate("file:///only", 10_000)],
            Vec::new(),
        )
        .expect_err("one candidate cannot be ambiguous");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    }

    #[test]
    fn optional_package_members_do_not_reduce_availability() {
        let essence = ResourceId::new();
        let thumbnail = ResourceId::new();
        let structure = ContentStructure::package(vec![
            ResourceMember::new(
                essence,
                ResourceRole::new("org.postproject:essence").expect("valid role"),
                true,
            ),
            ResourceMember::new(
                thumbnail,
                ResourceRole::new("org.postproject:thumbnail").expect("valid role"),
                false,
            ),
        ])
        .expect("valid package");

        let resolution = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &structure,
            vec![
                resource_result(thumbnail, ResourceResolutionState::Offline),
                resource_result(essence, ResourceResolutionState::OnlineAtKnownLocator),
            ],
        )
        .expect("matching results aggregate");

        assert_eq!(
            resolution.availability(),
            RepresentationAvailability::Online
        );
        assert_eq!(resolution.resources()[0].resource_id(), essence);
        assert_eq!(resolution.issues().len(), 1);
        assert!(!resolution.issues()[0].is_required());
    }

    #[test]
    fn some_missing_required_members_make_a_representation_partial() {
        let first = ResourceId::new();
        let second = ResourceId::new();
        let role = ResourceRole::new("org.postproject:essence").expect("valid role");
        let structure = ContentStructure::ordered_parts(vec![
            ResourceMember::new(first, role.clone(), true),
            ResourceMember::new(second, role, true),
        ])
        .expect("valid ordered parts");

        let resolution = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &structure,
            vec![
                resource_result(first, ResourceResolutionState::ResolvedExact),
                resource_result(second, ResourceResolutionState::Offline),
            ],
        )
        .expect("matching results aggregate");

        assert_eq!(
            resolution.availability(),
            RepresentationAvailability::Partial
        );
        assert_eq!(
            resolution.issues()[0].kind(),
            AvailabilityIssueKind::OfflineResource
        );
        assert!(resolution.issues()[0].is_required());
    }

    #[test]
    fn known_sequence_gaps_are_partial_with_frame_diagnostics() {
        let resource_id = ResourceId::new();
        let descriptor = ImageSequenceDescriptor::new(
            resource_id,
            FrameRange::new(1001, 1004, 1).expect("valid frame range"),
            RationalRate::new(24, 1).expect("valid rate"),
            vec![1002, 1003],
        )
        .expect("valid sequence");
        let structure = ContentStructure::image_sequence(descriptor);

        let resolution = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &structure,
            vec![resource_result(
                resource_id,
                ResourceResolutionState::OnlineAtKnownLocator,
            )],
        )
        .expect("matching results aggregate");

        assert_eq!(
            resolution.availability(),
            RepresentationAvailability::Partial
        );
        assert_eq!(
            resolution.issues()[0].kind(),
            AvailabilityIssueKind::MissingFrames
        );
        assert_eq!(resolution.issues()[0].frames(), [1002, 1003]);
    }

    #[test]
    fn observed_sequence_gaps_merge_with_recorded_exceptions() {
        let resource_id = ResourceId::new();
        let descriptor = ImageSequenceDescriptor::new(
            resource_id,
            FrameRange::new(1001, 1004, 1).expect("valid frame range"),
            RationalRate::new(24, 1).expect("valid rate"),
            vec![1002],
        )
        .expect("valid sequence");
        let structure = ContentStructure::image_sequence(descriptor);
        let resource = resource_result(resource_id, ResourceResolutionState::OnlineAtKnownLocator)
            .with_missing_frames(vec![1004, 1003, 1002])
            .expect("bounded frame diagnostics");

        let resolution = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &structure,
            vec![resource],
        )
        .expect("matching results aggregate");

        assert_eq!(
            resolution.availability(),
            RepresentationAvailability::Partial
        );
        assert_eq!(resolution.issues()[0].frames(), [1002, 1003, 1004]);
    }

    #[test]
    fn aggregate_rejects_the_wrong_resource_set() {
        let expected = ResourceId::new();
        let unexpected = ResourceId::new();
        let error = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &ContentStructure::single_resource(expected),
            vec![resource_result(
                unexpected,
                ResourceResolutionState::Offline,
            )],
        )
        .expect_err("unrelated resource must be rejected");

        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    }

    proptest! {
        #[test]
        fn ordering_is_invariant_under_input_reversal(
            left_score in 0_u16..=10_000,
            right_score in 0_u16..=10_000,
        ) {
            let id = ResourceId::from_bytes([4; 16]);
            let forward = ResourceResolution::new(
                id,
                ResourceResolutionState::Ambiguous,
                vec![candidate("file:///a", left_score), candidate("file:///b", right_score)],
                Vec::new(),
            ).expect("valid resolution");
            let reverse = ResourceResolution::new(
                id,
                ResourceResolutionState::Ambiguous,
                vec![candidate("file:///b", right_score), candidate("file:///a", left_score)],
                Vec::new(),
            ).expect("valid resolution");

            prop_assert_eq!(forward, reverse);
        }
    }
}
