"""Structured exceptions translated from stable C ABI error codes."""

from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum

from ._model import MediaRootId, ObjectReference, RevisionId


class ConflictKeyKind(IntEnum):
    """Stable category of a semantic transaction conflict key."""

    LOCATOR_SET = 1
    METADATA_PROPERTY = 2
    DEPENDENCY_SET = 3
    MEDIA_ROOT = 4
    EXTERNAL_IDENTIFIER = 5
    RESOURCE_FINGERPRINT = 6
    REPRESENTATION_FINGERPRINT = 7


@dataclass(frozen=True, slots=True)
class ConflictKey:
    """One non-mergeable semantic fact changed after a transaction base."""

    kind: ConflictKeyKind
    target: ObjectReference | MediaRootId
    namespace_name: str | None = None
    local_name: str | None = None
    qualifier: str | None = None
    version: int | None = None


@dataclass(frozen=True, slots=True)
class TransactionConflict:
    """Machine-readable detail for one optimistic transaction conflict."""

    key: ConflictKey
    base_revision_id: RevisionId | None
    base_revision_sequence: int
    superseding_revision_id: RevisionId
    superseding_revision_sequence: int


class PostProjectError(RuntimeError):
    """Base error reported by the native PostProject library."""

    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


class InvalidArgumentError(PostProjectError, ValueError):
    """An argument violates the public domain contract."""


class NotFoundError(PostProjectError):
    """A requested production object does not exist."""


class AlreadyExistsError(PostProjectError):
    """A unique production object or attachment already exists."""


class StorageError(PostProjectError):
    """Persistent production data could not be read or written safely."""


class IoError(PostProjectError, OSError):
    """A filesystem or operating-system operation failed."""


class MigrationError(PostProjectError):
    """A production schema could not be migrated safely."""


class ConflictError(PostProjectError):
    """An operation conflicts with current transaction or production state."""

    def __init__(
        self,
        code: int,
        message: str,
        conflict: TransactionConflict | None = None,
    ) -> None:
        super().__init__(code, message)
        self.conflict = conflict


class UnsupportedError(PostProjectError):
    """The requested operation is not supported by the current ABI."""


class AmbiguousResolutionError(PostProjectError):
    """A mutation requires an explicit choice between resolution candidates."""


class FingerprintError(PostProjectError):
    """Media fingerprinting failed."""


class CancelledError(PostProjectError):
    """The caller cancelled the operation through a cancellation token."""


class InternalError(PostProjectError):
    """The native library reported an internal invariant failure."""


ERROR_TYPES: dict[int, type[PostProjectError]] = {
    1: InvalidArgumentError,
    2: NotFoundError,
    3: AlreadyExistsError,
    4: IoError,
    5: StorageError,
    6: MigrationError,
    7: ConflictError,
    8: AmbiguousResolutionError,
    9: FingerprintError,
    10: UnsupportedError,
    11: CancelledError,
    255: InternalError,
}
