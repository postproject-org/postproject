"""Immutable Python values copied from the native ABI."""

from __future__ import annotations

import os
from dataclasses import dataclass
from enum import Enum
from typing import Generic, TypeAlias, TypeVar
from uuid import UUID


@dataclass(frozen=True, slots=True)
class _TypedId:
    value: UUID

    def __str__(self) -> str:
        return str(self.value)


class ProductionId(_TypedId):
    """Stable identity of one PostProject production."""

    __slots__ = ()


class AssetId(_TypedId):
    """Stable identity of one logical asset."""

    __slots__ = ()


@dataclass(frozen=True, slots=True)
class Asset:
    """Immutable logical asset summary."""

    id: AssetId
    created_at_unix_micros: int
    display_name: str | None
    import_source: str | None


class RepresentationId(_TypedId):
    """Stable identity of one usable asset representation."""

    __slots__ = ()


class ResourceId(_TypedId):
    """Stable identity of one storage resource."""

    __slots__ = ()


class LocatorId(_TypedId):
    """Stable identity of one resource locator."""

    __slots__ = ()


class MediaRootId(_TypedId):
    """Stable identity of one configured media root."""

    __slots__ = ()


@dataclass(frozen=True, slots=True)
class MediaRoot:
    """Immutable configured resolver root."""

    id: MediaRootId
    name: str
    label: str | None
    legacy_uri: str | None
    priority: int
    enabled: bool


class ActivityId(_TypedId):
    """Stable identity of one provenance activity."""

    __slots__ = ()


class JobId(_TypedId):
    """Stable identity of one durable production job."""

    __slots__ = ()


class JobClaimId(_TypedId):
    """Capability identifying one active job claim."""

    __slots__ = ()


class RevisionId(_TypedId):
    """Stable identity of one committed revision."""

    __slots__ = ()


class TransactionId(_TypedId):
    """Stable identity of the transaction that produced a revision."""

    __slots__ = ()


@dataclass(frozen=True, slots=True)
class CommittedRevision:
    """Identity and sequence produced by one successful commit."""

    id: RevisionId
    sequence: int


@dataclass(frozen=True, slots=True)
class CommitReceipt:
    """Atomic commit outcome; no revision means no new journal entry."""

    production_id: ProductionId
    revision: CommittedRevision | None


ObjectReference: TypeAlias = (
    ProductionId | AssetId | RepresentationId | ResourceId | ActivityId | JobId
)

QueryItem = TypeVar("QueryItem")


@dataclass(frozen=True, slots=True)
class QueryPage(Generic[QueryItem]):
    """One bounded page from a named domain query."""

    items: tuple[QueryItem, ...]
    next_cursor: str | None
    traversal_truncated: bool = False


@dataclass(frozen=True, slots=True)
class HostObjectBinding:
    """Portable production-scoped reference stored by a host application."""

    production_id: ProductionId
    object: ObjectReference


@dataclass(frozen=True, slots=True)
class ExternalIdentifier:
    """Opaque external identity preserved exactly as supplied."""

    scheme: str
    value: str
    qualifier: str | None = None


class DependencySetStatus(Enum):
    """Freshness of one complete dependency observation."""

    CURRENT = "current"
    NEEDS_EXTRACTION = "needs_extraction"


@dataclass(frozen=True, slots=True)
class Dependency:
    """One exact authored dependency edge."""

    kind: str
    target: AssetId | RepresentationId
    authored_reference: str
    required: bool = True
    source_resource_id: ResourceId | None = None
    resolved_representation_id: RepresentationId | None = None


@dataclass(frozen=True, slots=True)
class DependencySet:
    """One complete ordered dependency observation."""

    source_representation_id: RepresentationId
    recorded_at_revision: int
    status: DependencySetStatus
    dependencies: tuple[Dependency, ...]


@dataclass(frozen=True, slots=True)
class DependencyMatch:
    """One dependency-query target and its shortest observed depth."""

    target: AssetId | RepresentationId
    depth: int


@dataclass(frozen=True, slots=True)
class ProvenanceMatch:
    """One provenance-query representation and its shortest observed depth."""

    representation_id: RepresentationId
    depth: int


@dataclass(frozen=True, slots=True)
class FingerprintSnapshot:
    """Fingerprint evidence captured at one semantic revision."""

    algorithm: str
    version: int
    value: bytes
    observed_revision_sequence: int | None


@dataclass(frozen=True, slots=True)
class ActivityEdgeSnapshot:
    """Storage-owned fingerprint state captured when an activity committed."""

    revision_sequence: int
    fingerprints: tuple[FingerprintSnapshot, ...]


@dataclass(frozen=True, slots=True)
class ActivityEdge:
    representation_id: RepresentationId
    role: str | None = None
    snapshot: ActivityEdgeSnapshot | None = None


@dataclass(frozen=True, slots=True)
class ToolIdentity:
    name: str
    version: str | None = None
    uri: str | None = None


@dataclass(frozen=True, slots=True)
class AgentIdentity:
    name: str | None = None
    identifier: ExternalIdentifier | None = None


@dataclass(frozen=True, slots=True)
class Activity:
    id: ActivityId
    kind: str
    started_at_unix_micros: int | None
    finished_at_unix_micros: int | None
    tool: ToolIdentity | None
    agent: AgentIdentity | None
    inputs: tuple[ActivityEdge, ...]
    outputs: tuple[ActivityEdge, ...]


@dataclass(frozen=True, slots=True)
class ActivitySpec:
    kind: str
    outputs: tuple[ActivityEdge, ...]
    inputs: tuple[ActivityEdge, ...] = ()
    started_at_unix_micros: int | None = None
    finished_at_unix_micros: int | None = None
    tool: ToolIdentity | None = None
    agent: AgentIdentity | None = None


class ArtifactKnowledgeState(Enum):
    """Knowledge-only state of an activity-produced representation."""

    CURRENT = "current"
    STALE = "stale"
    INDETERMINATE = "indeterminate"
    DIVERGED = "diverged"


class ArtifactEdgeKind(Enum):
    """Side of an activity supplying fingerprint evidence."""

    INPUT = "input"
    OUTPUT = "output"


class ArtifactReasonKind(Enum):
    """Machine-readable explanation of a non-current artifact state."""

    PRODUCING_ACTIVITY_MISSING = "producing_activity_missing"
    PRODUCING_ACTIVITY_AMBIGUOUS = "producing_activity_ambiguous"
    SNAPSHOT_ABSENT = "snapshot_absent"
    FINGERPRINT_EVIDENCE_MISSING = "fingerprint_evidence_missing"
    FINGERPRINT_CHANGED = "fingerprint_changed"
    FINGERPRINT_RECOMPUTATION_PENDING = "fingerprint_recomputation_pending"
    UPSTREAM_NOT_CURRENT = "upstream_not_current"
    TRAVERSAL_TRUNCATED = "traversal_truncated"
    DEPENDENCY_SNAPSHOT_ABSENT = "dependency_snapshot_absent"
    DEPENDENCY_KNOWLEDGE_INCOMPLETE = "dependency_knowledge_incomplete"
    DEPENDENCY_PATH_CHANGED = "dependency_path_changed"
    DEPENDENCY_FINGERPRINT_CHANGED = "dependency_fingerprint_changed"
    DEPENDENCY_FINGERPRINT_RECOMPUTATION_PENDING = (
        "dependency_fingerprint_recomputation_pending"
    )
    DEPENDENCY_FINGERPRINT_EVIDENCE_MISSING = "dependency_fingerprint_evidence_missing"


class ArtifactDependencyIssue(Enum):
    """Why captured dependency knowledge cannot prove currentness."""

    NEEDS_EXTRACTION = "needs_extraction"
    UNRESOLVED = "unresolved"
    DEPTH_TRUNCATED = "depth_truncated"
    REPRESENTATIONS_TRUNCATED = "representations_truncated"


@dataclass(frozen=True, slots=True)
class ArtifactDependencyPathSegment:
    """One exact authored edge in a captured dependency path."""

    source_representation_id: RepresentationId
    dependency_position: int
    source_resource_id: ResourceId | None
    kind: str
    target: ObjectReference
    resolved_representation_id: RepresentationId | None
    authored_reference: str


class ArtifactTraversalLimit(Enum):
    """Explicit bound that stopped artifact evaluation."""

    DEPTH = "depth"
    REPRESENTATIONS = "representations"


@dataclass(frozen=True, slots=True)
class ArtifactReason:
    """One structured explanation of an artifact knowledge state."""

    kind: ArtifactReasonKind
    representation_id: RepresentationId
    activity_id: ActivityId | None = None
    edge_kind: ArtifactEdgeKind | None = None
    upstream_state: ArtifactKnowledgeState | None = None
    traversal_limit: ArtifactTraversalLimit | None = None
    activity_count: int | None = None
    fingerprint_algorithm: str | None = None
    fingerprint_version: int | None = None
    snapshot_value: bytes | None = None
    current_value: bytes | None = None
    input_representation_id: RepresentationId | None = None
    dependency_issue: ArtifactDependencyIssue | None = None
    dependency_path: tuple[ArtifactDependencyPathSegment, ...] = ()


@dataclass(frozen=True, slots=True)
class ArtifactEvaluation:
    """Complete, bounded evaluation of one artifact representation."""

    representation_id: RepresentationId
    state: ArtifactKnowledgeState
    visited_representations: int
    truncated: bool
    reasons: tuple[ArtifactReason, ...]


class ArtifactReproducibilityIssueKind(Enum):
    """Missing condition that prevents artifact reproduction."""

    PRODUCING_ACTIVITY_MISSING = "producing_activity_missing"
    PRODUCING_ACTIVITY_AMBIGUOUS = "producing_activity_ambiguous"
    TOOL_IDENTITY_MISSING = "tool_identity_missing"
    PARAMETERS_MISSING = "parameters_missing"
    INPUT_REPRESENTATION_MISSING = "input_representation_missing"


@dataclass(frozen=True, slots=True)
class ArtifactReproducibilityIssue:
    """One structured missing reproducibility condition."""

    kind: ArtifactReproducibilityIssueKind
    activity_id: ActivityId | None = None
    representation_id: RepresentationId | None = None
    activity_count: int | None = None


@dataclass(frozen=True, slots=True)
class ArtifactReproducibility:
    """Recorded knowledge needed to reproduce an artifact."""

    representation_id: RepresentationId
    reproducible: bool
    producing_activity_id: ActivityId | None
    activity_kind: str | None
    issues: tuple[ArtifactReproducibilityIssue, ...]


class RepresentationKind(Enum):
    """Semantic role of an asset representation."""

    ORIGINAL = "original"
    PROXY = "proxy"
    OPTIMIZED = "optimized"
    DERIVED = "derived"


class JobState(Enum):
    """Lifecycle state of durable requested work."""

    REQUESTED = "requested"
    CLAIMED = "claimed"
    SUCCEEDED = "succeeded"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass(frozen=True, slots=True)
class JobClaim:
    """Attribution and lease detail for one active claim."""

    id: JobClaimId
    tool: ToolIdentity
    agent: AgentIdentity | None
    expires_at_unix_micros: int


@dataclass(frozen=True, slots=True)
class JobCompletion:
    """Activity and representation committed by a successful job."""

    activity_id: ActivityId
    representation_id: RepresentationId


@dataclass(frozen=True, slots=True)
class Job:
    """One durable production-work request."""

    id: JobId
    kind: str
    inputs: tuple[RepresentationId, ...]
    output_asset_id: AssetId
    output_representation_kind: RepresentationKind
    target_root: str | None
    state: JobState
    claim: JobClaim | None
    completion: JobCompletion | None
    failure_diagnostic: str | None


@dataclass(frozen=True, slots=True)
class JobRequest:
    """New requested work to stage in a transaction."""

    kind: str
    inputs: tuple[RepresentationId, ...]
    output_asset_id: AssetId
    output_representation_kind: RepresentationKind
    target_root: str | None = None


class ContentStructureKind(Enum):
    """Structural shape used to realize a representation."""

    SINGLE_RESOURCE = "single_resource"
    IMAGE_SEQUENCE = "image_sequence"
    ORDERED_PARTS = "ordered_parts"
    PACKAGE = "package"


class LocatorAvailability(Enum):
    """Last observed availability of a resource locator."""

    UNKNOWN = "unknown"
    ONLINE = "online"
    OFFLINE = "offline"


@dataclass(frozen=True, slots=True)
class Fingerprint:
    """Versioned, opaque content-identity evidence."""

    algorithm: str
    version: int
    value: bytes


@dataclass(frozen=True, slots=True)
class RepresentationMember:
    resource_id: ResourceId
    role: str | None
    required: bool


@dataclass(frozen=True, slots=True)
class SequenceNaming:
    """How the files of an image sequence are named in one directory.

    A file is named by the prefix, the frame number zero-padded to at least
    ``padding`` digits, and the suffix. A naming belongs to a locator, not to
    the sequence: copies of one sequence may name their files differently.
    """

    prefix: str
    suffix: str
    padding: int

    def filename(self, frame: int) -> str:
        """Return the file name of one frame, without a directory."""

        sign = "-" if frame < 0 else ""
        digits = str(abs(frame)).rjust(max(self.padding - len(sign), 0), "0")
        return f"{self.prefix}{sign}{digits}{self.suffix}"


@dataclass(frozen=True, slots=True)
class LocatorIdentity:
    """Exact current locator evidence used to find known media.

    Image-sequence identities include the directory URI and naming. A
    directory URI without naming does not match a sequence.
    """

    uri: str
    sequence_naming: SequenceNaming | None = None


@dataclass(frozen=True, slots=True)
class ImageSequenceDescriptor:
    """What an image sequence is: frames, rate, and known gaps.

    Its file names belong to each locator (see :class:`SequenceNaming`).
    """

    start: int
    end: int
    step: int
    rate_numerator: int
    rate_denominator: int
    missing_frames: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class FileResourceInput:
    """Filesystem source and membership semantics for a compound member."""

    path: str
    role: str
    required: bool = True


@dataclass(frozen=True, slots=True)
class FileSource:
    """Media source for one regular file."""

    path: str | os.PathLike[str]


@dataclass(frozen=True, slots=True)
class ImageSequenceSource:
    """Media source for one compact image sequence in a directory.

    The directory and naming become the sequence's first locator.
    """

    directory: str | os.PathLike[str]
    naming: SequenceNaming
    start: int
    end: int
    step: int
    rate_numerator: int
    rate_denominator: int
    missing_frames: tuple[int, ...] = ()


@dataclass(frozen=True, slots=True)
class OrderedPartsSource:
    """Media source for ordered, fully required files, such as camera spans."""

    parts: tuple[FileResourceInput, ...]


@dataclass(frozen=True, slots=True)
class PackageSource:
    """Media source for role-bearing required and optional files."""

    members: tuple[FileResourceInput, ...]


MediaSource: TypeAlias = (
    FileSource | ImageSequenceSource | OrderedPartsSource | PackageSource
)
"""Content structure of a representation at its present location."""


@dataclass(frozen=True, slots=True)
class Locator:
    id: LocatorId
    uri: str
    availability: LocatorAvailability
    last_seen_unix_micros: int | None
    #: Naming of the files in the directory, exactly for a locator of an
    #: image-sequence resource.
    sequence_naming: SequenceNaming | None = None


@dataclass(frozen=True, slots=True)
class LocatorMatch:
    """One locator-query result with its owning resource and logical root."""

    resource_id: ResourceId
    locator: Locator
    media_root: str | None


@dataclass(frozen=True, slots=True)
class KnownMediaMatch:
    """One matching resource and its owning representation and asset."""

    asset_id: AssetId
    representation_id: RepresentationId
    resource_id: ResourceId


@dataclass(frozen=True, slots=True)
class Resource:
    id: ResourceId
    file_size: int | None
    modified_at_unix_micros: int | None
    fingerprints: tuple[Fingerprint, ...]
    locators: tuple[Locator, ...]


@dataclass(frozen=True, slots=True)
class Representation:
    id: RepresentationId
    asset_id: AssetId
    kind: RepresentationKind
    structure_kind: ContentStructureKind
    members: tuple[RepresentationMember, ...]
    image_sequence: ImageSequenceDescriptor | None
    fingerprints: tuple[Fingerprint, ...]
    resources: tuple[Resource, ...]


class RepresentationAvailability(Enum):
    """Aggregate availability of a complete representation."""

    ONLINE = "online"
    PARTIAL = "partial"
    OFFLINE = "offline"
    AMBIGUOUS = "ambiguous"
    ERROR = "error"


class ResourceResolutionState(Enum):
    """Outcome of resolving one storage resource."""

    ONLINE_AT_KNOWN_LOCATOR = "online_at_known_locator"
    RESOLVED_EXACT = "resolved_exact"
    RESOLVED_PROBABLE = "resolved_probable"
    OFFLINE = "offline"
    AMBIGUOUS = "ambiguous"
    ERROR = "error"


class AvailabilityIssueKind(Enum):
    """Machine-readable category of a representation availability issue."""

    OFFLINE_RESOURCE = "offline_resource"
    AMBIGUOUS_RESOURCE = "ambiguous_resource"
    RESOURCE_ERROR = "resource_error"
    MISSING_FRAMES = "missing_frames"


class VerificationMode(Enum):
    """Cost tier for resources found at a known locator."""

    PRESENCE = "presence"
    CONTENT = "content"


class ContentVerification(Enum):
    """Result of comparing present content with stored fingerprints."""

    MATCHES = "matches"
    DIFFERS = "differs"
    NOT_COMPARABLE = "not_comparable"


class ContentObservationOutcome(Enum):
    """How observed content relates to the stored fingerprints."""

    UNCHANGED = "unchanged"
    CHANGED = "changed"
    FIRST = "first"


class EvidenceKind(Enum):
    """Machine-readable reason supporting or opposing a candidate."""

    KNOWN_LOCATOR_AVAILABLE = "known_locator_available"
    EXACT_FINGERPRINT_MATCH = "exact_fingerprint_match"
    FULL_HASH_MATCH = "full_hash_match"
    PARTIAL_FINGERPRINT_MATCH = "partial_fingerprint_match"
    FILE_SIZE_MATCH = "file_size_match"
    FILE_NAME_MATCH = "file_name_match"
    RELATIVE_PATH_SIMILARITY = "relative_path_similarity"
    MEDIA_ROOT_RELATION = "media_root_relation"
    MEDIA_ROOT_UNMAPPED = "media_root_unmapped"
    MEDIA_ROOT_UNAVAILABLE = "media_root_unavailable"
    CONFLICTING_CANDIDATE = "conflicting_candidate"
    DISCOVERY_ERROR = "discovery_error"
    SEARCH_TRUNCATED = "search_truncated"
    FINGERPRINT_MISMATCH = "fingerprint_mismatch"
    FINGERPRINT_NOT_VERIFIED = "fingerprint_not_verified"


@dataclass(frozen=True, slots=True)
class ResolutionEvidence:
    kind: EvidenceKind
    detail: str | None = None


@dataclass(frozen=True, slots=True)
class ResolutionCandidate:
    uri: str
    confidence_basis_points: int
    #: Logical root the candidate was found under; ``None`` for a candidate
    #: found only in an unnamed search directory.
    media_root: str | None
    #: Naming the image-sequence files were found under; confirm the
    #: candidate with it. ``None`` for any other resource.
    sequence_naming: SequenceNaming | None
    evidence: tuple[ResolutionEvidence, ...]


@dataclass(frozen=True, slots=True)
class ResourceResolution:
    resource_id: ResourceId
    state: ResourceResolutionState
    candidates: tuple[ResolutionCandidate, ...]
    evidence: tuple[ResolutionEvidence, ...]


@dataclass(frozen=True, slots=True)
class AvailabilityIssue:
    resource_id: ResourceId
    required: bool
    kind: AvailabilityIssueKind
    frames: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class RepresentationResolution:
    asset_id: AssetId
    representation_id: RepresentationId
    availability: RepresentationAvailability
    resources: tuple[ResourceResolution, ...]
    issues: tuple[AvailabilityIssue, ...]


@dataclass(frozen=True, slots=True)
class MetadataProperty:
    """Vocabulary-qualified metadata property identity."""

    vocabulary: str
    property: str


@dataclass(frozen=True, slots=True)
class MetadataString:
    value: str


@dataclass(frozen=True, slots=True)
class MetadataLanguageString:
    value: str
    language: str


@dataclass(frozen=True, slots=True)
class MetadataI64:
    value: int


@dataclass(frozen=True, slots=True)
class MetadataU64:
    value: int


@dataclass(frozen=True, slots=True)
class MetadataDecimal:
    coefficient: int
    scale: int


@dataclass(frozen=True, slots=True)
class MetadataBool:
    value: bool


@dataclass(frozen=True, slots=True)
class MetadataTimestamp:
    unix_micros: int


@dataclass(frozen=True, slots=True)
class MetadataUri:
    value: str


@dataclass(frozen=True, slots=True)
class MetadataBytes:
    value: bytes


@dataclass(frozen=True, slots=True)
class MetadataRational:
    numerator: int
    denominator: int


@dataclass(frozen=True, slots=True)
class MetadataList:
    values: tuple[MetadataValue, ...]


@dataclass(frozen=True, slots=True)
class MetadataStructField:
    name: str
    value: MetadataValue


@dataclass(frozen=True, slots=True)
class MetadataStruct:
    fields: tuple[MetadataStructField, ...]


@dataclass(frozen=True, slots=True)
class MetadataReference:
    target: ObjectReference


MetadataValue: TypeAlias = (
    MetadataString
    | MetadataLanguageString
    | MetadataI64
    | MetadataU64
    | MetadataDecimal
    | MetadataBool
    | MetadataTimestamp
    | MetadataUri
    | MetadataBytes
    | MetadataRational
    | MetadataList
    | MetadataStruct
    | MetadataReference
)


@dataclass(frozen=True, slots=True)
class MetadataAssertion:
    target: ObjectReference
    property: MetadataProperty
    value: MetadataValue


@dataclass(frozen=True, slots=True)
class RegenerationJobPlan:
    """Read-only job proposal derived from an artifact's provenance."""

    artifact_representation_id: RepresentationId
    job: Job
    parameters: tuple[MetadataAssertion, ...]


@dataclass(frozen=True, slots=True)
class AssetImportedEvent:
    asset_id: AssetId


@dataclass(frozen=True, slots=True)
class RepresentationAddedEvent:
    asset_id: AssetId
    representation_id: RepresentationId


@dataclass(frozen=True, slots=True)
class ResourceAddedEvent:
    resource_id: ResourceId


@dataclass(frozen=True, slots=True)
class RepresentationResourceAddedEvent:
    representation_id: RepresentationId
    resource_id: ResourceId
    structural_position: int


@dataclass(frozen=True, slots=True)
class LocatorAddedEvent:
    resource_id: ResourceId
    locator_id: LocatorId


@dataclass(frozen=True, slots=True)
class LocatorRetiredEvent:
    resource_id: ResourceId
    locator_id: LocatorId


@dataclass(frozen=True, slots=True)
class MediaRootAddedEvent:
    media_root_id: MediaRootId


@dataclass(frozen=True, slots=True)
class MediaRootEnabledChangedEvent:
    media_root_id: MediaRootId
    enabled: bool


@dataclass(frozen=True, slots=True)
class MediaRootRemovedEvent:
    media_root_id: MediaRootId


@dataclass(frozen=True, slots=True)
class ExternalIdentifierAddedEvent:
    target: ObjectReference
    identifier: ExternalIdentifier


@dataclass(frozen=True, slots=True)
class ExternalIdentifierRemovedEvent:
    target: ObjectReference
    identifier: ExternalIdentifier


@dataclass(frozen=True, slots=True)
class MetadataAddedOrReplacedEvent:
    target: ObjectReference
    property: MetadataProperty


@dataclass(frozen=True, slots=True)
class MetadataRemovedEvent:
    target: ObjectReference
    property: MetadataProperty


@dataclass(frozen=True, slots=True)
class ActivityCreatedEvent:
    activity_id: ActivityId
    kind: str


@dataclass(frozen=True, slots=True)
class ActivityInputAddedEvent:
    activity_id: ActivityId
    representation_id: RepresentationId
    role: str | None = None


@dataclass(frozen=True, slots=True)
class ActivityOutputAddedEvent:
    activity_id: ActivityId
    representation_id: RepresentationId
    role: str | None = None


@dataclass(frozen=True, slots=True)
class ResourceFingerprintObservedEvent:
    resource_id: ResourceId
    algorithm: str
    version: int


@dataclass(frozen=True, slots=True)
class RepresentationFingerprintObservedEvent:
    representation_id: RepresentationId
    algorithm: str
    version: int


@dataclass(frozen=True, slots=True)
class DependencySetRecordedEvent:
    representation_id: RepresentationId


@dataclass(frozen=True, slots=True)
class JobRequestedEvent:
    job_id: JobId


@dataclass(frozen=True, slots=True)
class JobClaimedEvent:
    job_id: JobId


@dataclass(frozen=True, slots=True)
class JobClaimRenewedEvent:
    job_id: JobId


@dataclass(frozen=True, slots=True)
class JobClaimReleasedEvent:
    job_id: JobId


@dataclass(frozen=True, slots=True)
class JobSucceededEvent:
    job_id: JobId


@dataclass(frozen=True, slots=True)
class JobFailedEvent:
    job_id: JobId


@dataclass(frozen=True, slots=True)
class JobCancelledEvent:
    job_id: JobId


RevisionEventPayload: TypeAlias = (
    AssetImportedEvent
    | RepresentationAddedEvent
    | ResourceAddedEvent
    | RepresentationResourceAddedEvent
    | LocatorAddedEvent
    | LocatorRetiredEvent
    | MediaRootAddedEvent
    | MediaRootEnabledChangedEvent
    | MediaRootRemovedEvent
    | ExternalIdentifierAddedEvent
    | ExternalIdentifierRemovedEvent
    | MetadataAddedOrReplacedEvent
    | MetadataRemovedEvent
    | ActivityCreatedEvent
    | ActivityInputAddedEvent
    | ActivityOutputAddedEvent
    | ResourceFingerprintObservedEvent
    | RepresentationFingerprintObservedEvent
    | DependencySetRecordedEvent
    | JobRequestedEvent
    | JobClaimedEvent
    | JobClaimRenewedEvent
    | JobClaimReleasedEvent
    | JobSucceededEvent
    | JobFailedEvent
    | JobCancelledEvent
)


@dataclass(frozen=True, slots=True)
class RevisionEvent:
    """One ordered semantic event within a revision."""

    position: int
    payload: RevisionEventPayload


@dataclass(frozen=True, slots=True)
class OriginIdentity:
    """Integrating application or process identity, not an authenticated user."""

    name: str
    version: str | None = None
    uri: str | None = None


@dataclass(frozen=True, slots=True)
class RevisionContext:
    """Optional context applied to a transaction's future revision."""

    origin: OriginIdentity | None = None
    message: str | None = None


@dataclass(frozen=True, slots=True)
class Revision:
    """One committed production mutation transaction."""

    id: RevisionId
    sequence: int
    transaction_id: TransactionId
    committed_at_unix_micros: int
    origin: OriginIdentity | None
    message: str | None


class RevisionWaitResult(Enum):
    """Why a revision wait returned."""

    REVISIONS = "revisions"
    TIMED_OUT = "timed_out"
    CLOSED = "closed"
    CANCELLED = "cancelled"


@dataclass(frozen=True, slots=True)
class RevisionWait:
    """Outcome of one bounded revision wait.

    ``revisions`` is non-empty only for ``RevisionWaitResult.REVISIONS``.
    Closed and cancelled results are terminal for the waiter.
    """

    result: RevisionWaitResult
    revisions: tuple[Revision, ...]


@dataclass(frozen=True, slots=True)
class FilteredRevisionPage:
    """Matching revisions and the cursor for the next filtered page.

    Every matching revision with a sequence up to ``through_sequence`` is in
    ``revisions``.
    """

    revisions: tuple[Revision, ...]
    through_sequence: int
