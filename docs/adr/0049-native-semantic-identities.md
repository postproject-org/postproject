# 0049: Native semantic identities

Status: accepted for development; family migration is in progress.

## Decision

Project each UUID-backed domain identity as a distinct C struct and explicit
C++ value type. Keep its 16 persisted bytes unchanged. Small scalar inputs
use values; output pointers transfer stack values. Generic UUIDs remain an
explicit interchange boundary, with no implicit conversion between kinds.
C++ values support equality, ordering and standard containers.

Construction and parsing establish an identity's kind and UUID syntax, not
its existence or scope. Retain the core's acceptance of every 16-byte value,
including nil; store operations reject absent or wrongly scoped objects.
Do not infer validity from zero initialization or a pointer cast. Empty optional
identities use explicit state, rather than interpreting nil as absence.

Migrate one complete identity family at a time, including C/Rust/ctypes layouts,
native wrappers, examples and downstreams. Signature changes require a new ABI
version and recompilation. Other families remain explicitly incomplete during
development; the final candidate cannot keep generic IDs in semantic APIs.

## Migration and standards impact

This changes source contracts, not production files, external identifiers or
host binding strings. Existing published artifacts and the 0.6 C++ Result
protocol remain intact. Reviewed against the standards policy: no vocabulary,
identifier scheme, normalization or interchange mapping changes. UUID parsing
continues to use the existing core implementation.
