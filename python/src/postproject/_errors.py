"""Structured exceptions translated from stable C ABI error codes."""


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
