# ADR 0032: C++ error reporting without exceptions

- Status: Accepted; accessor names amended by ADR 0033
- Date: 2026-09-27

## Context

The header-only C++17 wrapper reported every failure by throwing
`postproject::Error`. In a few places it threw `std::runtime_error` or
`std::invalid_argument` instead. The C ABI is exception-free, but the wrapper
could not be compiled at all with exceptions disabled.

Many C++ hosts disable exceptions:

- KDE's CMake settings build application code with `-fno-exceptions`.
- Unreal Engine and Chromium, and many console and embedded toolchains, forbid
  exceptions.
- Plugin SDKs such as OpenFX must not let exceptions cross their boundaries.

In the first real-host integration, the one adapter file had to be recompiled
with exceptions enabled, which is the first thing a maintainer asks about. The
C ABI remained available, but it is too verbose to be what PostProject offers a
C++ host.

## Decision

The wrapper is Result-first:

- **`postproject::Result<T>`.** Every fallible operation returns this C++17
  value type, which holds either a `T` or a `postproject::Error`. It offers
  `ok()`, `error()`, unchecked `*` and `->`, `valueOr()`, and `value()`. The
  type is `[[nodiscard]]`. `Result<void>` covers operations that return
  nothing.
- **`Error` is a value.** It carries an `ErrorCode` and a message and is no
  longer an exception type.
- **One throw, only with exceptions.** When exceptions are enabled, detected
  through `__cpp_exceptions` or MSVC's `_CPPUNWIND`, `value()` throws
  `postproject::Exception`, a `std::runtime_error` that carries the `Error`.
  Otherwise `value()` on an error prints the error and aborts. No other code in
  the header throws.
- **One code path per operation.** The throwing style is only `value()` over
  the same `Result`, so no operation is implemented twice.
- **Deferred errors in builders.** `MetadataInput` and `ResolutionOptions` are
  built fluently. A factory or setter with invalid input records its first
  error, and the operation that consumes the builder returns it. Literal inputs
  therefore need no unwrapping.
- **Observer failures.** `RevisionObserver` reports a failed query as a
  `std::optional<Error>`. With exceptions enabled, it also catches an exception
  thrown by the host's callback and reports it as `ErrorCode::internal`.

A consumer test builds against the installed package with `-fno-exceptions`
(`/EHs-c-` on MSVC).

## Alternatives considered

- **A parallel non-throwing API** (`tryX` next to `X`, or a `nothrow`
  namespace). This doubles the surface and every example, and still leaves a
  header with `throw` expressions that exception-free builds cannot compile.
- **An error callback or a global error slot.** Neither lets the caller handle
  a failure where it happens, and global state breaks the threading contract of
  ADR 0014.
- **`std::expected`.** It needs C++23, and the wrapper targets C++17.

## Standards impact

None.

## Consequences

C++ callers change shape: the result of each call is either checked or
unwrapped with `.value()`, and `catch (postproject::Error)` becomes
`catch (postproject::Exception)`. Hosts built without exceptions use the
wrapper directly. Builders report invalid input when they are used, not when
they are built.
