# ADR 0008: Python binding over the C ABI

- Status: Accepted; amended by ADR 0039
- Date: 2026-09-20

## Decision

The Python package will consume only the installed public C ABI, use explicit
native-library discovery and cleanup, and duplicate no domain validation. The
binding technology is the Python standard library's `ctypes` module.

The ctypes signature and struct table is generated from the authoritative
header by `tools/generate_abi.py`, committed, and regenerated in CI so a stale
file fails the build. The header remains the single source of truth; the
generator encodes no ABI knowledge of its own, runs only at development and CI
time, and fails loudly on a declaration it cannot represent. Public struct sizes
and field offsets are additionally asserted against the values the C compiler
reports for the installed header, because a mirrored struct whose fields are
reordered or padded differently cannot be detected by symbol checks.

The wrapper requires an explicit native-library path, either as an API argument
or through `POSTPROJECT_LIBRARY`. Paths are expanded and resolved before
loading; the wrapper does not search the current working directory or mutate
the platform loader path. Packagers may supply an absolute bundled-library path
from their own installation layout. A platform wheel carries its own library,
which the binding loads when neither an argument nor the variable names one
(ADR 0039).

Every owned opaque C handle has one Python owner with deterministic `close()`
and context-manager cleanup plus an idempotent finalizer fallback. Borrowed
strings and result-set values are copied before the owning native handle is
released. Error codes become structured Python exceptions.

## Consequences

Python validates the same integration boundary used by non-Rust applications.
PyO3 and direct internal Rust access are excluded from the primary binding.

Using `ctypes` avoids an additional runtime or extension-compilation dependency
and works on Linux, macOS, and Windows. It also preserves the explicit
library-path policy above, which a build-time-linked extension module such as
CFFI in API mode would not: that approach buys compiler-verified declarations at
the cost of per-platform wheels, a build toolchain, and runtime discovery.

The residual tradeoff is that the binding restates the header in a second
language, where a mistake corrupts memory rather than raising. Generating the
table and cross-checking layout against the compiler removes the hand-maintained
step; the ABI version check remains the guard against a mismatched library.

This constrains the public header to remain mechanically parseable — fixed-width
typedefs, opaque handle pointers, flat structs, no unions, varargs, or function
pointers. A declaration the generator cannot represent is a reason to simplify
the header rather than to special-case the generator.
