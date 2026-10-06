# 0048: Immediate native option and token validation

Status: accepted for the 0.7 development API.

## Decision

C++ cancellation-token creation returns `Result<CancelToken>`. Allocation or
native errors cannot silently produce a token that never cancels. Tokens
remain move-only; cancelling a moved-from token is a harmless no-op.

Resolution options are created through a fallible factory. Each setter returns
`Result<void>` immediately and leaves the previous valid options unchanged on
failure. A failed setter does not poison later resolution or suppress later
valid setters. A moved-from options object cannot mean default options.
The C boundary owns the validation used by every projection; multi-field
updates validate all fields before changing any of them.

Python checks integer types and representable ranges before `ctypes` conversion;
booleans are not integers for these arguments. This covers query bounds,
fingerprint versions, sequence values, metadata numbers and timestamps.
Native validation still enforces domain constraints such as positive rates.

## Migration and standards impact

Use `CancelToken::create()` and `ResolutionOptions::create()`, then check each
setter with the established Result protocol or `.value()` in exception-oriented
code. The 0.6 C++ Result propagation promise and released artifacts remain
unchanged. This supersedes the deferred-options behavior described in ADR 0032
for 0.7. No schema, identifier or external standards mapping changes; reviewed
against the contributor standards policy.
