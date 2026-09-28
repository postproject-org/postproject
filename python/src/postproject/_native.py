"""Secure native-library loading and common ABI error handling."""

from __future__ import annotations

import ctypes
import os
import threading
from _ctypes import _Pointer
from pathlib import Path

from ._abi import Error, configure_api
from ._errors import ERROR_TYPES, PostProjectError

ABI_VERSION = 33
LIBRARY_ENVIRONMENT_VARIABLE = "POSTPROJECT_LIBRARY"


_loaded: dict[Path, NativeLibrary] = {}
_loading = threading.Lock()


class NativeLibrary:
    """One explicitly located PostProject shared library.

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
        if error:
            raw_message = self.lib.pp_error_message(error)
            if raw_message:
                message = raw_message.decode("utf-8", errors="replace")
            self.lib.pp_error_release(error)
        error_type = ERROR_TYPES.get(status, PostProjectError)
        raise error_type(status, message)


def _library_path(path: str | os.PathLike[str] | None) -> Path:
    supplied = (
        path if path is not None else os.environ.get(LIBRARY_ENVIRONMENT_VARIABLE)
    )
    if supplied is None:
        raise RuntimeError(
            "pass library_path or set POSTPROJECT_LIBRARY to the native shared library"
        )
    resolved = Path(supplied).expanduser().resolve(strict=True)
    if not resolved.is_file():
        raise RuntimeError(f"PostProject library is not a file: {resolved}")
    return resolved
