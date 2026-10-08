"""Immutable Python values copied from the native ABI."""

from __future__ import annotations

import os
from collections.abc import Iterable
from dataclasses import dataclass, fields
from enum import Enum
from fractions import Fraction
from itertools import islice
from typing import ClassVar, Generic, TypeAlias, TypeVar
from uuid import UUID

from ._ids import (
    ActivityId,
    AssetId,
    JobId,
    LocatorId,
    MediaRootId,
    ProductionId,
    RepresentationId,
    ResourceId,
    RevisionId,
    TransactionId,
)


@dataclass(frozen=True, slots=True)
class Asset:
    """Immutable logical asset summary."""

    id: AssetId
    created_at_unix_micros: int
    display_name: str | None
    import_source: str | None


@dataclass(frozen=True, slots=True)
class MediaRoot:
    """Immutable configured resolver root."""

    id: MediaRootId
    name: str
    label: str | None
    legacy_uri: str | None
    priority: int
    enabled: bool


@dataclass(frozen=True, slots=True)
class CommittedRevision:
    """Identity and sequence produced by one successful commit."""

    id: RevisionId
    sequence: int


@dataclass(frozen=True, slots=True)
class DecisionBase:
    """Detached production-scoped decision context, without a retained view."""

    production_id: ProductionId
    revision: CommittedRevision | None

    @classmethod
    def from_token(
        cls, token: str, *, library_path: str | os.PathLike[str] | None = None
    ) -> DecisionBase:
        """Parse canonical scoped context; editing still validates store membership."""
        from ._production import _parse_decision_base

        return _parse_decision_base(token, library_path)

    def to_token(self, *, library_path: str | os.PathLike[str] | None = None) -> str:
        """Export context without retaining a view, lock or permission."""
        from ._production import _format_decision_base

        return _format_decision_base(self, library_path)


@dataclass(frozen=True, slots=True)
class CommitReceipt:
    """Atomic commit outcome; no revision means no new journal entry."""

    production_id: ProductionId
    revision: CommittedRevision | None


@dataclass(frozen=True, slots=True)
class ProductionRef:
    """Explicit production target for polymorphic operations."""

    id: ProductionId

    def __post_init__(self) -> None:
        if not isinstance(self.id, UUID):
            raise TypeError("reference identity must be a uuid.UUID")


@dataclass(frozen=True, slots=True)
class AssetRef:
    """Explicit asset target for polymorphic operations."""

    id: AssetId

    def __post_init__(self) -> None:
        if not isinstance(self.id, UUID):
            raise TypeError("reference identity must be a uuid.UUID")


@dataclass(frozen=True, slots=True)
class RepresentationRef:
    """Explicit representation target for polymorphic operations."""

    id: RepresentationId

    def __post_init__(self) -> None:
        if not isinstance(self.id, UUID):
            raise TypeError("reference identity must be a uuid.UUID")


@dataclass(frozen=True, slots=True)
class ResourceRef:
    """Explicit resource target for polymorphic operations."""

    id: ResourceId

    def __post_init__(self) -> None:
        if not isinstance(self.id, UUID):
            raise TypeError("reference identity must be a uuid.UUID")


@dataclass(frozen=True, slots=True)
class ActivityRef:
    """Explicit activity target for polymorphic operations."""

    id: ActivityId

    def __post_init__(self) -> None:
        if not isinstance(self.id, UUID):
            raise TypeError("reference identity must be a uuid.UUID")


@dataclass(frozen=True, slots=True)
class JobRef:
    """Explicit job target for polymorphic operations."""

    id: JobId

    def __post_init__(self) -> None:
        if not isinstance(self.id, UUID):
            raise TypeError("reference identity must be a uuid.UUID")


ObjectReference: TypeAlias = (
    ProductionRef | AssetRef | RepresentationRef | ResourceRef | ActivityRef | JobRef
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
    target: AssetRef | RepresentationRef
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

    target: AssetRef | RepresentationRef
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
class _ArtifactReasonValue:
    """Validate and own a case payload before exposing an observation."""

    def __post_init__(self) -> None:
        enum_fields = {
            "edge_kind": ArtifactEdgeKind,
            "upstream_state": ArtifactKnowledgeState,
            "traversal_limit": ArtifactTraversalLimit,
            "dependency_issue": ArtifactDependencyIssue,
        }
        for field in fields(self):
            value = getattr(self, field.name)
            if field.name.endswith("_id") and not isinstance(value, UUID):
                raise TypeError(f"{field.name} must be a UUID")
            if field.name in enum_fields and not isinstance(
                value, enum_fields[field.name]
            ):
                raise TypeError(f"{field.name} has the wrong enum type")
            if field.name == "dependency_path":
                copied = tuple(islice(value, 100001))
                if len(copied) > 100000:
                    raise ValueError("dependency path exceeds its item limit")
                if any(
                    not isinstance(segment, ArtifactDependencyPathSegment)
                    for segment in copied
                ):
                    raise TypeError("dependency path requires typed segments")
                object.__setattr__(self, field.name, copied)
            if field.name in ("snapshot_value", "current_value"):
                optional = isinstance(
                    self,
                    (
                        ArtifactFingerprintEvidenceMissing,
                        ArtifactDependencyFingerprintEvidenceMissing,
                    ),
                )
                if value is None and optional:
                    continue
                if not isinstance(value, (bytes, bytearray, memoryview)):
                    raise TypeError(f"{field.name} must be bytes")
                size = value.nbytes if isinstance(value, memoryview) else len(value)
                if not 1 <= size <= 16 * 1024 * 1024:
                    raise ValueError(
                        "fingerprint value must contain 1 byte through 16 MiB"
                    )
                object.__setattr__(self, field.name, bytes(value))
        if hasattr(self, "activity_count"):
            count = self.activity_count
            if not isinstance(count, int) or isinstance(count, bool):
                raise TypeError("activity_count must be an integer")
            if not 2 <= count <= 2**32 - 1:
                raise ValueError("ambiguous producer count must be 2..2**32-1")
        if hasattr(self, "fingerprint_algorithm"):
            algorithm = self.fingerprint_algorithm
            version = getattr(self, "fingerprint_version", None)
            optional = isinstance(
                self,
                (
                    ArtifactFingerprintEvidenceMissing,
                    ArtifactDependencyFingerprintEvidenceMissing,
                ),
            )
            if algorithm is None and version is None and optional:
                return
            if (
                not isinstance(algorithm, str)
                or not isinstance(version, int)
                or isinstance(version, bool)
            ):
                raise TypeError(
                    "fingerprint domain requires an algorithm and integer version"
                )
            if (
                not 0 <= version <= 65535
                or not 1 <= len(algorithm) <= 64
                or any(
                    not (
                        character.isascii()
                        and (character.isalnum() or character in "-_")
                    )
                    for character in algorithm
                )
            ):
                raise ValueError("invalid fingerprint domain")
        if (
            isinstance(self, ArtifactUpstreamNotCurrent)
            and self.upstream_state is ArtifactKnowledgeState.CURRENT
        ):
            raise ValueError(
                "upstream-not-current evidence cannot report current knowledge"
            )


@dataclass(frozen=True, slots=True)
class ArtifactProducerMissing(_ArtifactReasonValue):
    """producing activity missing."""

    representation_id: RepresentationId
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.PRODUCING_ACTIVITY_MISSING


@dataclass(frozen=True, slots=True)
class ArtifactProducerAmbiguous(_ArtifactReasonValue):
    """producing activity ambiguous."""

    representation_id: RepresentationId
    activity_count: int
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.PRODUCING_ACTIVITY_AMBIGUOUS


@dataclass(frozen=True, slots=True)
class ArtifactSnapshotAbsent(_ArtifactReasonValue):
    """snapshot absent."""

    activity_id: ActivityId
    representation_id: RepresentationId
    edge_kind: ArtifactEdgeKind
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.SNAPSHOT_ABSENT


@dataclass(frozen=True, slots=True)
class ArtifactFingerprintEvidenceMissing(_ArtifactReasonValue):
    """fingerprint evidence missing."""

    activity_id: ActivityId
    representation_id: RepresentationId
    edge_kind: ArtifactEdgeKind
    fingerprint_algorithm: str | None
    fingerprint_version: int | None
    snapshot_value: bytes | None
    current_value: bytes | None
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.FINGERPRINT_EVIDENCE_MISSING


@dataclass(frozen=True, slots=True)
class ArtifactFingerprintChanged(_ArtifactReasonValue):
    """fingerprint changed."""

    activity_id: ActivityId
    representation_id: RepresentationId
    edge_kind: ArtifactEdgeKind
    fingerprint_algorithm: str
    fingerprint_version: int
    snapshot_value: bytes
    current_value: bytes
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.FINGERPRINT_CHANGED


@dataclass(frozen=True, slots=True)
class ArtifactFingerprintRecomputationPending(_ArtifactReasonValue):
    """fingerprint recomputation pending."""

    activity_id: ActivityId
    representation_id: RepresentationId
    edge_kind: ArtifactEdgeKind
    kind: ClassVar[ArtifactReasonKind] = (
        ArtifactReasonKind.FINGERPRINT_RECOMPUTATION_PENDING
    )


@dataclass(frozen=True, slots=True)
class ArtifactUpstreamNotCurrent(_ArtifactReasonValue):
    """upstream not current."""

    representation_id: RepresentationId
    upstream_state: ArtifactKnowledgeState
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.UPSTREAM_NOT_CURRENT


@dataclass(frozen=True, slots=True)
class ArtifactTraversalTruncated(_ArtifactReasonValue):
    """traversal truncated."""

    representation_id: RepresentationId
    traversal_limit: ArtifactTraversalLimit
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.TRAVERSAL_TRUNCATED


@dataclass(frozen=True, slots=True)
class ArtifactDependencySnapshotAbsent(_ArtifactReasonValue):
    """dependency snapshot absent."""

    activity_id: ActivityId
    representation_id: RepresentationId
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.DEPENDENCY_SNAPSHOT_ABSENT


@dataclass(frozen=True, slots=True)
class ArtifactDependencyKnowledgeIncomplete(_ArtifactReasonValue):
    """dependency knowledge incomplete."""

    activity_id: ActivityId
    input_representation_id: RepresentationId
    representation_id: RepresentationId
    dependency_path: tuple[ArtifactDependencyPathSegment, ...]
    dependency_issue: ArtifactDependencyIssue
    kind: ClassVar[ArtifactReasonKind] = (
        ArtifactReasonKind.DEPENDENCY_KNOWLEDGE_INCOMPLETE
    )


@dataclass(frozen=True, slots=True)
class ArtifactDependencyPathChanged(_ArtifactReasonValue):
    """dependency path changed."""

    activity_id: ActivityId
    input_representation_id: RepresentationId
    dependency_path: tuple[ArtifactDependencyPathSegment, ...]
    kind: ClassVar[ArtifactReasonKind] = ArtifactReasonKind.DEPENDENCY_PATH_CHANGED


@dataclass(frozen=True, slots=True)
class ArtifactDependencyFingerprintChanged(_ArtifactReasonValue):
    """dependency fingerprint changed."""

    activity_id: ActivityId
    input_representation_id: RepresentationId
    representation_id: RepresentationId
    dependency_path: tuple[ArtifactDependencyPathSegment, ...]
    fingerprint_algorithm: str
    fingerprint_version: int
    snapshot_value: bytes
    current_value: bytes
    kind: ClassVar[ArtifactReasonKind] = (
        ArtifactReasonKind.DEPENDENCY_FINGERPRINT_CHANGED
    )


@dataclass(frozen=True, slots=True)
class ArtifactDependencyFingerprintRecomputationPending(_ArtifactReasonValue):
    """dependency fingerprint recomputation pending."""

    activity_id: ActivityId
    input_representation_id: RepresentationId
    representation_id: RepresentationId
    dependency_path: tuple[ArtifactDependencyPathSegment, ...]
    kind: ClassVar[ArtifactReasonKind] = (
        ArtifactReasonKind.DEPENDENCY_FINGERPRINT_RECOMPUTATION_PENDING
    )


@dataclass(frozen=True, slots=True)
class ArtifactDependencyFingerprintEvidenceMissing(_ArtifactReasonValue):
    """dependency fingerprint evidence missing."""

    activity_id: ActivityId
    input_representation_id: RepresentationId
    representation_id: RepresentationId
    dependency_path: tuple[ArtifactDependencyPathSegment, ...]
    fingerprint_algorithm: str | None
    fingerprint_version: int | None
    snapshot_value: bytes | None
    current_value: bytes | None
    kind: ClassVar[ArtifactReasonKind] = (
        ArtifactReasonKind.DEPENDENCY_FINGERPRINT_EVIDENCE_MISSING
    )


ArtifactReason: TypeAlias = (
    ArtifactProducerMissing
    | ArtifactProducerAmbiguous
    | ArtifactSnapshotAbsent
    | ArtifactFingerprintEvidenceMissing
    | ArtifactFingerprintChanged
    | ArtifactFingerprintRecomputationPending
    | ArtifactUpstreamNotCurrent
    | ArtifactTraversalTruncated
    | ArtifactDependencySnapshotAbsent
    | ArtifactDependencyKnowledgeIncomplete
    | ArtifactDependencyPathChanged
    | ArtifactDependencyFingerprintChanged
    | ArtifactDependencyFingerprintRecomputationPending
    | ArtifactDependencyFingerprintEvidenceMissing
)


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
class ReproducibilityProducerMissing:
    """No activity records how the artifact was produced."""

    kind: ClassVar[ArtifactReproducibilityIssueKind] = (
        ArtifactReproducibilityIssueKind.PRODUCING_ACTIVITY_MISSING
    )


@dataclass(frozen=True, slots=True)
class ReproducibilityProducerAmbiguous:
    """Several activities claim to have produced the artifact."""

    activity_count: int
    kind: ClassVar[ArtifactReproducibilityIssueKind] = (
        ArtifactReproducibilityIssueKind.PRODUCING_ACTIVITY_AMBIGUOUS
    )

    def __post_init__(self) -> None:
        if not isinstance(self.activity_count, int) or isinstance(
            self.activity_count, bool
        ):
            raise TypeError("activity_count must be an integer")
        if not 2 <= self.activity_count <= 2**32 - 1:
            raise ValueError("ambiguous producer count must be 2..2**32-1")


@dataclass(frozen=True, slots=True)
class ReproducibilityToolMissing:
    """The producing activity has no tool identity."""

    activity_id: ActivityId
    kind: ClassVar[ArtifactReproducibilityIssueKind] = (
        ArtifactReproducibilityIssueKind.TOOL_IDENTITY_MISSING
    )

    def __post_init__(self) -> None:
        if not isinstance(self.activity_id, UUID):
            raise TypeError("activity_id must be a UUID")


@dataclass(frozen=True, slots=True)
class ReproducibilityParametersMissing:
    """The producing activity has no recorded parameters."""

    activity_id: ActivityId
    kind: ClassVar[ArtifactReproducibilityIssueKind] = (
        ArtifactReproducibilityIssueKind.PARAMETERS_MISSING
    )

    def __post_init__(self) -> None:
        if not isinstance(self.activity_id, UUID):
            raise TypeError("activity_id must be a UUID")


@dataclass(frozen=True, slots=True)
class ReproducibilityInputMissing:
    """A producing activity references an absent input representation."""

    activity_id: ActivityId
    representation_id: RepresentationId
    kind: ClassVar[ArtifactReproducibilityIssueKind] = (
        ArtifactReproducibilityIssueKind.INPUT_REPRESENTATION_MISSING
    )

    def __post_init__(self) -> None:
        if not isinstance(self.activity_id, UUID) or not isinstance(
            self.representation_id, UUID
        ):
            raise TypeError("activity_id and representation_id must be UUIDs")


ArtifactReproducibilityIssue: TypeAlias = (
    ReproducibilityProducerMissing
    | ReproducibilityProducerAmbiguous
    | ReproducibilityToolMissing
    | ReproducibilityParametersMissing
    | ReproducibilityInputMissing
)


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
class PendingJobLease:
    """Ownership belongs to an edit that has not committed."""


@dataclass(frozen=True, slots=True)
class ActiveJobLease:
    """Cached accepted expiry; mutations still recheck authoritative state."""

    expires_at_unix_micros: int


@dataclass(frozen=True, slots=True)
class ClosedJobLease:
    """Ownership ended; freeing the handle remains necessary."""


JobLeaseStatus: TypeAlias = PendingJobLease | ActiveJobLease | ClosedJobLease


@dataclass(frozen=True, slots=True)
class JobClaim:
    """Attribution and lease detail for one active claim."""

    tool: ToolIdentity
    agent: AgentIdentity | None
    expires_at_unix_micros: int


@dataclass(frozen=True, slots=True)
class JobCompletion:
    """Activity and representation committed by a successful job."""

    activity_id: ActivityId
    representation_id: RepresentationId


@dataclass(frozen=True, slots=True)
class JobRequested:
    """A request with no current worker or result."""


@dataclass(frozen=True, slots=True)
class JobFailure:
    """A failed request carrying its bounded diagnostic."""

    diagnostic: str

    def __post_init__(self) -> None:
        if not isinstance(self.diagnostic, str):
            raise TypeError("job failure diagnostic must be str")
        if (
            not self.diagnostic
            or "\0" in self.diagnostic
            or len(self.diagnostic.encode("utf-8")) > 4096
        ):
            raise ValueError(
                "job failure diagnostic must contain 1-4096 UTF-8 bytes without NUL"
            )


@dataclass(frozen=True, slots=True)
class JobCancelled:
    """A request cancelled by its coordinator."""


JobStatus: TypeAlias = (
    JobRequested | JobClaim | JobCompletion | JobFailure | JobCancelled
)


@dataclass(frozen=True, slots=True)
class Job:
    """One durable production-work request."""

    id: JobId
    kind: str
    inputs: tuple[RepresentationId, ...]
    output_asset_id: AssetId
    output_representation_kind: RepresentationKind
    target_root: str | None
    status: JobStatus

    def __post_init__(self) -> None:
        if not isinstance(
            self.status,
            (JobRequested, JobClaim, JobCompletion, JobFailure, JobCancelled),
        ):
            raise TypeError("job status must be a supported lifecycle alternative")
        object.__setattr__(self, "inputs", tuple(self.inputs))

    @property
    def state(self) -> JobState:
        """Category of the active status alternative."""
        if isinstance(self.status, JobRequested):
            return JobState.REQUESTED
        if isinstance(self.status, JobClaim):
            return JobState.CLAIMED
        if isinstance(self.status, JobCompletion):
            return JobState.SUCCEEDED
        if isinstance(self.status, JobFailure):
            return JobState.FAILED
        return JobState.CANCELLED

    @property
    def claim(self) -> JobClaim | None:
        """Claim detail when claimed; otherwise None."""
        return self.status if isinstance(self.status, JobClaim) else None

    @property
    def completion(self) -> JobCompletion | None:
        """Completion detail when succeeded; otherwise None."""
        return self.status if isinstance(self.status, JobCompletion) else None

    @property
    def failure_diagnostic(self) -> str | None:
        """Diagnostic when failed; otherwise None."""
        return self.status.diagnostic if isinstance(self.status, JobFailure) else None


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

    def __post_init__(self) -> None:
        if not isinstance(self.resource_id, UUID) or not isinstance(
            self.required, bool
        ):
            raise TypeError(
                "membership requires a UUID resource and a bool required flag"
            )
        if self.role is not None:
            if not isinstance(self.role, str):
                raise TypeError("member role must be str or None")
            namespace, separator, local = self.role.partition(":")
            if (
                not separator
                or not namespace
                or not local
                or len(self.role) > 128
                or not self.role.isascii()
                or any(not (char.isalnum() or char in "._-:") for char in self.role)
            ):
                raise ValueError("member role must be a namespaced ASCII identifier")


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

    def __post_init__(self) -> None:
        for value, minimum, maximum in (
            (self.start, -(2**63), 2**63 - 1),
            (self.end, -(2**63), 2**63 - 1),
            (self.step, 1, 2**32 - 1),
            (self.rate_numerator, 1, 2**32 - 1),
            (self.rate_denominator, 1, 2**32 - 1),
        ):
            if isinstance(value, bool) or not isinstance(value, int):
                raise TypeError("sequence descriptor numbers must be int")
            if not minimum <= value <= maximum:
                raise ValueError("sequence descriptor number is outside its domain")
        if self.end < self.start or (self.end - self.start) % self.step:
            raise ValueError("sequence end must be ascending and aligned to the step")
        missing = tuple(islice(self.missing_frames, 100001))
        if len(missing) > 100000:
            raise ValueError("sequence has more than 100000 sparse exceptions")
        for frame in missing:
            if isinstance(frame, bool) or not isinstance(frame, int):
                raise TypeError("missing frames must be int")
            if not self.start <= frame <= self.end or (frame - self.start) % self.step:
                raise ValueError("missing frame is outside the stepped domain")
        object.__setattr__(self, "missing_frames", tuple(sorted(set(missing))))

    @property
    def rate(self) -> Fraction:
        """Exact sequence rate as a standard Fraction."""
        return Fraction(self.rate_numerator, self.rate_denominator)


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
    ``rate`` is a positive exact :class:`fractions.Fraction`.
    """

    directory: str | os.PathLike[str]
    naming: SequenceNaming
    start: int
    end: int
    step: int
    rate: Fraction
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
class SingleResourceContent:
    """One required storage resource."""

    resource_id: ResourceId

    def __post_init__(self) -> None:
        if not isinstance(self.resource_id, UUID):
            raise TypeError("content resource must be a uuid.UUID")


@dataclass(frozen=True, slots=True)
class ImageSequenceContent:
    """One patterned resource with a compact sequence descriptor."""

    resource_id: ResourceId
    descriptor: ImageSequenceDescriptor

    def __post_init__(self) -> None:
        if not isinstance(self.resource_id, UUID) or not isinstance(
            self.descriptor, ImageSequenceDescriptor
        ):
            raise TypeError("sequence content requires a UUID resource and descriptor")


def _content_members(
    members: Iterable[RepresentationMember], *, ordered: bool
) -> tuple[RepresentationMember, ...]:
    copied = tuple(islice(members, 100001))
    if not 1 <= len(copied) <= 100000:
        raise ValueError("compound content must contain 1-100000 members")
    if any(not isinstance(member, RepresentationMember) for member in copied):
        raise TypeError("content members must be RepresentationMember values")
    if len({member.resource_id for member in copied}) != len(copied):
        raise ValueError("content cannot contain duplicate resource identities")
    if any(member.role is None for member in copied):
        raise ValueError("compound content members require roles")
    if ordered and any(not member.required for member in copied):
        raise ValueError("ordered content members must all be required")
    if not any(member.required for member in copied):
        raise ValueError("content must have at least one required member")
    return copied


@dataclass(frozen=True, slots=True, init=False)
class OrderedPartsContent:
    """Ordered required resources, with unique identities and explicit roles."""

    members: tuple[RepresentationMember, ...]

    def __init__(self, members: Iterable[RepresentationMember]) -> None:
        object.__setattr__(self, "members", _content_members(members, ordered=True))


@dataclass(frozen=True, slots=True, init=False)
class PackageContent:
    """Required and optional resources, with at least one required member."""

    members: tuple[RepresentationMember, ...]

    def __init__(self, members: Iterable[RepresentationMember]) -> None:
        object.__setattr__(self, "members", _content_members(members, ordered=False))


RepresentationContent: TypeAlias = (
    SingleResourceContent | ImageSequenceContent | OrderedPartsContent | PackageContent
)


@dataclass(frozen=True, slots=True)
class Representation:
    id: RepresentationId
    asset_id: AssetId
    kind: RepresentationKind
    content: RepresentationContent
    fingerprints: tuple[Fingerprint, ...]
    resources: tuple[Resource, ...]

    def __post_init__(self) -> None:
        if not isinstance(
            self.content,
            (
                SingleResourceContent,
                ImageSequenceContent,
                OrderedPartsContent,
                PackageContent,
            ),
        ):
            raise TypeError("representation content must be a supported alternative")
        object.__setattr__(self, "fingerprints", tuple(self.fingerprints))
        object.__setattr__(self, "resources", tuple(self.resources))

    @property
    def structure_kind(self) -> ContentStructureKind:
        """Category derived from the stored content alternative."""
        if isinstance(self.content, SingleResourceContent):
            return ContentStructureKind.SINGLE_RESOURCE
        if isinstance(self.content, ImageSequenceContent):
            return ContentStructureKind.IMAGE_SEQUENCE
        if isinstance(self.content, OrderedPartsContent):
            return ContentStructureKind.ORDERED_PARTS
        return ContentStructureKind.PACKAGE

    @property
    def members(self) -> tuple[RepresentationMember, ...]:
        """Membership derived from the content alternative."""
        if isinstance(self.content, (SingleResourceContent, ImageSequenceContent)):
            return (RepresentationMember(self.content.resource_id, None, True),)
        return self.content.members

    @property
    def image_sequence(self) -> ImageSequenceDescriptor | None:
        """Sequence descriptor exactly for sequence content."""
        return (
            self.content.descriptor
            if isinstance(self.content, ImageSequenceContent)
            else None
        )


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
    """Copied diagnostic evidence with a known category."""

    kind: EvidenceKind
    detail: str | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.kind, EvidenceKind):
            raise TypeError("evidence kind must be an EvidenceKind")
        if self.detail is not None and not isinstance(self.detail, str):
            raise TypeError("evidence detail must be text or None")


def _resolution_evidence(
    supplied: Iterable[ResolutionEvidence],
) -> tuple[ResolutionEvidence, ...]:
    evidence = tuple(islice(supplied, 100_001))
    if len(evidence) > 100_000:
        raise ValueError("resolution evidence exceeds 100000 items")
    if any(not isinstance(item, ResolutionEvidence) for item in evidence):
        raise TypeError("resolution evidence must contain ResolutionEvidence values")
    return evidence


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

    def __post_init__(self) -> None:
        if not isinstance(self.uri, str) or not self.uri:
            raise TypeError("candidate URI must be nonempty text")
        if isinstance(self.confidence_basis_points, bool) or not isinstance(
            self.confidence_basis_points, int
        ):
            raise TypeError("candidate confidence must be an integer")
        if not 0 <= self.confidence_basis_points <= 10_000:
            raise ValueError("candidate confidence must be in 0..10000")
        if self.media_root is not None and not isinstance(self.media_root, str):
            raise TypeError("candidate media root must be text or None")
        if self.sequence_naming is not None and not isinstance(
            self.sequence_naming, SequenceNaming
        ):
            raise TypeError("candidate naming must be SequenceNaming or None")
        evidence = _resolution_evidence(self.evidence)
        if not evidence:
            raise ValueError("candidate must contain evidence")
        object.__setattr__(self, "evidence", evidence)


@dataclass(frozen=True, slots=True)
class _SingleResolutionCandidate:
    candidate: ResolutionCandidate

    def __post_init__(self) -> None:
        if not isinstance(self.candidate, ResolutionCandidate):
            raise TypeError("resolution requires one validated candidate")


@dataclass(frozen=True, slots=True)
class ResourceOnlineAtKnownLocator(_SingleResolutionCandidate):
    """One persisted locator is available."""

    state: ClassVar[ResourceResolutionState] = (
        ResourceResolutionState.ONLINE_AT_KNOWN_LOCATOR
    )


@dataclass(frozen=True, slots=True)
class ResourceResolvedExact(_SingleResolutionCandidate):
    """One candidate has exact identity evidence."""

    state: ClassVar[ResourceResolutionState] = ResourceResolutionState.RESOLVED_EXACT


@dataclass(frozen=True, slots=True)
class ResourceResolvedProbable(_SingleResolutionCandidate):
    """One candidate requires a caller decision before confirmation."""

    state: ClassVar[ResourceResolutionState] = ResourceResolutionState.RESOLVED_PROBABLE


@dataclass(frozen=True, slots=True)
class ResourceOffline:
    """No credible candidate was found."""

    state: ClassVar[ResourceResolutionState] = ResourceResolutionState.OFFLINE


@dataclass(frozen=True, slots=True)
class ResourceAmbiguous:
    """Two or more candidates require an explicit caller decision."""

    candidates: tuple[ResolutionCandidate, ...]
    state: ClassVar[ResourceResolutionState] = ResourceResolutionState.AMBIGUOUS

    def __post_init__(self) -> None:
        candidates = tuple(islice(self.candidates, 100_001))
        if not 2 <= len(candidates) <= 100_000:
            raise ValueError("ambiguous resolution requires 2..100000 candidates")
        if any(not isinstance(item, ResolutionCandidate) for item in candidates):
            raise TypeError("resolution candidates must be validated values")
        object.__setattr__(self, "candidates", candidates)


@dataclass(frozen=True, slots=True)
class ResourceResolutionFailure:
    """Discovery or verification could not complete safely."""

    state: ClassVar[ResourceResolutionState] = ResourceResolutionState.ERROR


ResolutionOutcome: TypeAlias = (
    ResourceOnlineAtKnownLocator
    | ResourceResolvedExact
    | ResourceResolvedProbable
    | ResourceOffline
    | ResourceAmbiguous
    | ResourceResolutionFailure
)


@dataclass(frozen=True, slots=True)
class ResourceResolution:
    """Owned resource facts with one checked resolution outcome."""

    resource_id: ResourceId
    outcome: ResolutionOutcome
    evidence: tuple[ResolutionEvidence, ...]

    def __post_init__(self) -> None:
        if not isinstance(self.resource_id, UUID):
            raise TypeError("resource identity must be a UUID")
        if not isinstance(
            self.outcome,
            (
                ResourceOnlineAtKnownLocator,
                ResourceResolvedExact,
                ResourceResolvedProbable,
                ResourceOffline,
                ResourceAmbiguous,
                ResourceResolutionFailure,
            ),
        ):
            raise TypeError("resolution outcome must be a supported alternative")
        object.__setattr__(self, "evidence", _resolution_evidence(self.evidence))

    @property
    def state(self) -> ResourceResolutionState:
        """Display category derived from the outcome."""
        return self.outcome.state

    @property
    def candidates(self) -> tuple[ResolutionCandidate, ...]:
        """Applicable candidates derived from the outcome."""
        if isinstance(self.outcome, _SingleResolutionCandidate):
            return (self.outcome.candidate,)
        if isinstance(self.outcome, ResourceAmbiguous):
            return self.outcome.candidates
        return ()


@dataclass(frozen=True, slots=True)
class OfflineResourceIssue:
    """The resource has no credible locator."""

    kind: ClassVar[AvailabilityIssueKind] = AvailabilityIssueKind.OFFLINE_RESOURCE


@dataclass(frozen=True, slots=True)
class AmbiguousResourceIssue:
    """Resource candidates need a caller decision."""

    kind: ClassVar[AvailabilityIssueKind] = AvailabilityIssueKind.AMBIGUOUS_RESOURCE


@dataclass(frozen=True, slots=True)
class ResourceErrorIssue:
    """Resource discovery or verification failed."""

    kind: ClassVar[AvailabilityIssueKind] = AvailabilityIssueKind.RESOURCE_ERROR


@dataclass(frozen=True, slots=True)
class MissingSequenceFrames:
    """Owned, sorted missing-frame observations."""

    frames: tuple[int, ...]
    kind: ClassVar[AvailabilityIssueKind] = AvailabilityIssueKind.MISSING_FRAMES

    def __post_init__(self) -> None:
        frames = tuple(islice(self.frames, 100_001))
        if not 1 <= len(frames) <= 100_000:
            raise ValueError("missing-frame issue requires 1..100000 frames")
        for frame in frames:
            if isinstance(frame, bool) or not isinstance(frame, int):
                raise TypeError("missing frames must be integers")
            if not -(2**63) <= frame <= 2**63 - 1:
                raise ValueError("missing frame is outside signed 64-bit range")
        object.__setattr__(self, "frames", tuple(sorted(set(frames))))


AvailabilityIssueDetail: TypeAlias = (
    OfflineResourceIssue
    | AmbiguousResourceIssue
    | ResourceErrorIssue
    | MissingSequenceFrames
)


@dataclass(frozen=True, slots=True)
class AvailabilityIssue:
    """Resource-specific diagnostic with only its applicable payload."""

    resource_id: ResourceId
    required: bool
    detail: AvailabilityIssueDetail

    def __post_init__(self) -> None:
        if not isinstance(self.resource_id, UUID):
            raise TypeError("resource identity must be a UUID")
        if not isinstance(self.required, bool):
            raise TypeError("required must be bool")
        if not isinstance(
            self.detail,
            (
                OfflineResourceIssue,
                AmbiguousResourceIssue,
                ResourceErrorIssue,
                MissingSequenceFrames,
            ),
        ):
            raise TypeError("availability detail must be a supported alternative")

    @property
    def kind(self) -> AvailabilityIssueKind:
        """Display category derived from the diagnostic detail."""
        return self.detail.kind

    @property
    def frames(self) -> tuple[int, ...]:
        """Missing frames exactly for a frame-specific diagnostic."""
        return (
            self.detail.frames if isinstance(self.detail, MissingSequenceFrames) else ()
        )


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
class ResourceFileFactsObservedEvent:
    resource_id: ResourceId


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
    | ResourceFileFactsObservedEvent
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
