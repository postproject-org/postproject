"""Copied source-scoped exchange facts, without worker credentials."""

from __future__ import annotations

from dataclasses import dataclass, fields
from enum import IntEnum
from typing import TYPE_CHECKING, TypeAlias

from ._errors import PostProjectError
from ._ids import ClientId, HistoryId, JobId, ProductionId, RequestId
from ._model import (
    CommitReceipt,
    DecisionBase,
    JobCancelled,
    JobCompletion,
    JobRequested,
)

if TYPE_CHECKING:
    from ._production import JobLease


@dataclass(frozen=True, slots=True)
class ProtocolBase:
    """Detached decision retaining its original source history."""

    decision: DecisionBase
    history_id: HistoryId


@dataclass(frozen=True, slots=True)
class ExchangePosition:
    """One complete source boundary and its exact BLAKE3 commitment."""

    base: ProtocolBase
    digest: bytes

    def __post_init__(self) -> None:
        if not isinstance(self.digest, bytes) or len(self.digest) != 32:
            raise ValueError("exchange digest must contain exactly 32 bytes")


@dataclass(frozen=True, slots=True)
class RequestIdentity:
    """Complete public scope to retain after an uncertain response."""

    production_id: ProductionId
    history_id: HistoryId
    client_id: ClientId
    request_id: RequestId


@dataclass(frozen=True, slots=True)
class ExchangeLimits:
    """Positive receiver budgets; container depth is at most 192."""

    file_bytes: int = 1 << 30
    disk_bytes: int = 2 << 30
    frames: int = 10_000_000
    document_bytes: int = 64 << 20
    document_depth: int = 192
    document_nodes: int = 1_000_000

    def __post_init__(self) -> None:
        for field in fields(self):
            value = getattr(self, field.name)
            maximum = 192 if field.name == "document_depth" else 2**64 - 1
            if type(value) is not int or not 0 < value <= maximum:
                raise ValueError(f"{field.name} must be an integer in 1..{maximum}")


class ExchangeRole(IntEnum):
    """Persisted execution authority; replay never promotes a mirror."""

    AUTHORITY = 1
    PASSIVE_MIRROR = 2


@dataclass(frozen=True, slots=True)
class ExchangeState:
    """One coherent observation, including a possibly incomplete native head."""

    protocol_version: int
    role: ExchangeRole
    supported_features: int
    observed_base: ProtocolBase
    floor: ExchangePosition
    head: ExchangePosition | None
    limits: ExchangeLimits


@dataclass(frozen=True, slots=True)
class ClaimedJobResult:
    """Original authority-selected expiry; grants no worker ownership."""

    expires_at_unix_micros: int


@dataclass(frozen=True, slots=True)
class FailedJobResult:
    """Failure observed at the receipt; diagnostics remain in committed history."""


JobResultStatus: TypeAlias = (
    JobRequested | ClaimedJobResult | JobCompletion | FailedJobResult | JobCancelled
)


@dataclass(frozen=True, slots=True)
class JobResult:
    """Final public job observation at this submission's own receipt."""

    job_id: JobId
    status: JobResultStatus


@dataclass(frozen=True, slots=True)
class SubmissionOutcome:
    """Copied accepted receipt or terminal rejection, without ownership."""

    status: CommitReceipt | PostProjectError
    jobs: list[JobResult]
    document: bytes


@dataclass(frozen=True, slots=True)
class SubmissionResult:
    """Public facts and freshly delivered owning leases; duplicates have none."""

    outcome: SubmissionOutcome
    leases: list[JobLease]
