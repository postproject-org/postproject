"""Secure native-library loading and common ABI error handling."""

from __future__ import annotations

import ctypes
import os
import sys
import threading
from _ctypes import _Pointer
from pathlib import Path

from ._abi import Error, configure_api
from ._errors import ERROR_TYPES, PostProjectError

ABI_VERSION = 36
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
        if error:
            raw_message = self.lib.pp_error_message(error)
            if raw_message:
                message = raw_message.decode("utf-8", errors="replace")
            self.lib.pp_error_release(error)
        error_type = ERROR_TYPES.get(status, PostProjectError)
        raise error_type(status, message)


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
