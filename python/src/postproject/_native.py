"""Secure native-library loading and common ABI error handling."""

from __future__ import annotations

import ctypes
import os
import sys
import threading
from _ctypes import _Pointer
from pathlib import Path
from uuid import UUID

from . import _abi
from ._abi import Error, configure_api
from ._abi import TransactionConflict as NativeTransactionConflict
from ._errors import (
    ERROR_TYPES,
    ConflictError,
    ConflictKey,
    ConflictKeyKind,
    PostProjectError,
    TransactionConflict,
)
from ._model import (
    ActivityId,
    AssetId,
    JobId,
    MediaRootId,
    ProductionId,
    RepresentationId,
    ResourceId,
    RevisionId,
)

ABI_VERSION = 38
LIBRARY_ENVIRONMENT_VARIABLE = "POSTPROJECT_LIBRARY"
#: Where a platform wheel installs its native library (ADR 0039).
BUNDLED_LIBRARY = (
    Path(__file__).parent
    / "_lib"
    / {"win32": "postproject.dll", "darwin": "libpostproject.dylib"}.get(
        sys.platform, "libpostproject.so"
    )
)


_loaded: dict[Path, NativeLibrary] = {}
_loading = threading.Lock()


class NativeLibrary:
    """One located PostProject shared library.

    The library is the explicit ``path``, else ``POSTPROJECT_LIBRARY``, else
    the one a platform wheel installed inside this package. No other place is
    searched.

    A library is loaded and checked once per resolved path; constructing it
    again returns the same instance.
    """

    path: Path
    lib: ctypes.CDLL

    def __new__(cls, path: str | os.PathLike[str] | None = None) -> NativeLibrary:
        resolved = _library_path(path)
        with _loading:
            library = _loaded.get(resolved)
            if library is None:
                library = super().__new__(cls)
                library.path = resolved
                library.lib = ctypes.CDLL(str(resolved))
                configure_api(library.lib)
                version = int(library.lib.pp_abi_version())
                if version != ABI_VERSION:
                    raise RuntimeError(
                        f"PostProject ABI {version} is incompatible with required "
                        f"ABI {ABI_VERSION}"
                    )
                _loaded[resolved] = library
        return library

    def check(self, status: int, error: _Pointer[Error]) -> None:
        """Release an optional native error and raise its Python equivalent."""

        if status == 0:
            if error:
                self.lib.pp_error_release(error)
            return
        message = "PostProject operation failed"
        conflict = None
        if error:
            try:
                raw_message = self.lib.pp_error_message(error)
                if raw_message:
                    message = raw_message.decode("utf-8", errors="replace")
                native_conflict = NativeTransactionConflict()
                if self.lib.pp_error_transaction_conflict(
                    error, ctypes.byref(native_conflict)
                ):
                    conflict = _transaction_conflict(native_conflict)
            finally:
                self.lib.pp_error_release(error)
        error_type = ERROR_TYPES.get(status, PostProjectError)
        if error_type is ConflictError:
            raise ConflictError(status, message, conflict)
        raise error_type(status, message)


def _transaction_conflict(native: NativeTransactionConflict) -> TransactionConflict:
    kind = ConflictKeyKind(int(native.kind))
    identifier = UUID(bytes=bytes(native.target.id.bytes))
    if kind is ConflictKeyKind.MEDIA_ROOT:
        target = MediaRootId(identifier)
    else:
        target_types = {
            _abi.PP_OBJECT_PRODUCTION: ProductionId,
            _abi.PP_OBJECT_ASSET: AssetId,
            _abi.PP_OBJECT_REPRESENTATION: RepresentationId,
            _abi.PP_OBJECT_RESOURCE: ResourceId,
            _abi.PP_OBJECT_ACTIVITY: ActivityId,
            _abi.PP_OBJECT_JOB: JobId,
        }
        target_type = target_types.get(int(native.target.kind))
        if target_type is None:
            raise RuntimeError("native transaction conflict has an unknown target kind")
        target = target_type(identifier)
    fingerprint = kind in {
        ConflictKeyKind.RESOURCE_FINGERPRINT,
        ConflictKeyKind.REPRESENTATION_FINGERPRINT,
    }
    return TransactionConflict(
        key=ConflictKey(
            kind=kind,
            target=target,
            namespace_name=_optional_text(native.namespace_name),
            local_name=_optional_text(native.local_name),
            qualifier=_optional_text(native.qualifier),
            version=int(native.version) if fingerprint else None,
        ),
        base_revision_id=(
            RevisionId(UUID(bytes=bytes(native.base_revision_id.bytes)))
            if native.has_base_revision
            else None
        ),
        base_revision_sequence=int(native.base_revision_sequence),
        superseding_revision_id=RevisionId(
            UUID(bytes=bytes(native.superseding_revision_id.bytes))
        ),
        superseding_revision_sequence=int(native.superseding_revision_sequence),
    )


def _optional_text(value: bytes | None) -> str | None:
    return value.decode("utf-8", errors="replace") if value is not None else None


def _library_path(path: str | os.PathLike[str] | None) -> Path:
    """Return the explicit path, else the variable, else the bundled library."""

    supplied = (
        path if path is not None else os.environ.get(LIBRARY_ENVIRONMENT_VARIABLE)
    )
    if supplied is None:
        if BUNDLED_LIBRARY.is_file():
            return BUNDLED_LIBRARY.resolve()
        raise RuntimeError(
            "pass library_path, set POSTPROJECT_LIBRARY to the native shared "
            "library, or install a platform wheel that contains it"
        )
    resolved = Path(supplied).expanduser().resolve(strict=True)
    if not resolved.is_file():
        raise RuntimeError(f"PostProject library is not a file: {resolved}")
    return resolved
