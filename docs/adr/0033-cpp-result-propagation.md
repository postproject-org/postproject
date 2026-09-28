# ADR 0033: Propagating C++ results

- Status: Accepted
- Date: 2026-09-28

## Context

ADR 0032 made the C++17 wrapper Result-first so that hosts built without
exceptions can use it. The first host to adopt it showed the cost. Most of the
adapter's functions call several PostProject operations and pass any failure
to their own caller. Without exceptions, every such call is followed by the
same three lines:

```cpp
if (!result.ok()) {
  return std::move(result).takeError();
}
```

The adapter has 22 of these. They account for about half of its growth since
it stopped using exceptions, and they make the code harder to read than the
version that relied on one `try` block. Every C++ host built without
exceptions writes the same pattern at every call. That includes KDE
applications, game engines, and plugins behind C plugin interfaces such as
OpenFX.

Hosts that allow exceptions are not affected: `value()` throws, and one `try`
block covers a whole function. Python hosts, such as Blender add-ons, use the
Python binding, which raises exceptions.

The header already defines `POSTPROJECT_TRY` and `POSTPROJECT_TRY_ASSIGN` to
propagate errors in one line, but it uses them only internally and undefines
them at its end.

`Result` also uses its own names for accessors that C++23 standardized in
`std::expected`, such as `ok()` for `has_value()` and `valueOr()` for
`value_or()`. Integrators who know `std::expected` or `std::optional` must learn
a second set of names for the same operations. Pre-1.0 hosts cannot move to
C++23 without every PostProject call site changing.

## Decision

**Propagation macros are public.** The header keeps the following macros
defined and documents them as supported API:

- `POSTPROJECT_TRY(expression)` evaluates a `Result` and, if it holds an error,
  returns that `Error` from the enclosing function.
- `POSTPROJECT_TRY_ASSIGN(declaration, expression)` does the same, and
  otherwise declares a variable holding the value. For example:
  `POSTPROJECT_TRY_ASSIGN(auto production, Production::open(path));`

The enclosing function must return a type that is implicitly constructible from
`postproject::Error`. Any `postproject::Result<U>` qualifies. A host can
propagate into its own error type by giving that type such a constructor.
`POSTPROJECT_TRY_ASSIGN` expands to several statements, so it may only be used
as a statement directly inside a block, and only once per source line. Its
declared variable belongs to the enclosing scope. Both macros work with and
without exceptions and use only standard C++17.

**`Result` uses the `std::expected` vocabulary.** The accessors and combinators
take the names and meanings of their C++23 `std::expected` counterparts, and
the wrapper's own names are removed:

| Before | After |
| --- | --- |
| `ok()` | `has_value()`; `explicit operator bool` stays |
| `valueOr(fallback)` | `value_or(fallback)` |
| `std::move(result).takeError()` | `std::move(result).error()` |

The following combinators are added, with the same rules as in
`std::expected`:

- `and_then(f)`: `f` takes the value and returns a `Result<U>`.
- `transform(f)`: `f` takes the value and returns a `U`.
- `or_else(f)`: `f` takes the `Error` and returns a `Result<T>`.

`Result<void>` has every member that `std::expected<void, E>` has, except
`value_or`. Domain operations keep the wrapper's camelCase names. Only the
vocabulary type follows the standard.

Two differences from `std::expected` remain and are documented:

- The error type is always `postproject::Error`. There is no
  `transform_error`.
- `Result` is constructed implicitly from an `Error`, without
  `std::unexpected`. The propagation macros rely on this.

As before, `value()` on an error throws `postproject::Exception` when
exceptions are enabled and otherwise aborts.

## Alternatives considered

- **Leave propagation to hosts.** Each host would write its own copy of the
  macros, and every example in the documentation would keep the three-line
  checks. The header already contains correct, tested versions of the macros.
- **Combinators only, without macros.** In C++17, chains of lambdas are harder
  to read than explicit checks once a function makes more than two calls or
  needs an intermediate value in a later call.
- **A transaction that records its first staging error and turns later calls
  into no-ops**, as `MetadataInput` and `ResolutionOptions` already do. Adapters
  read from the production between staging calls, so reads would still need
  checks. It would also hide which call failed, and a failed staging call is a
  signal to roll back (see the lifecycle guide).
- **Keep the camelCase names and add `andThen`, `transform`, and `orElse`.**
  This avoids renaming existing calls, but keeps a vocabulary only PostProject
  uses and rules out making `Result` an alias of `std::expected` later.
- **Make `Result` an alias of `std::expected<T, Error>` now.** This needs C++23,
  and the wrapper targets C++17 (ADR 0032).

## Standards impact

None. The change affects only the header-only C++ wrapper. The C ABI, the
schema, and the domain model are unchanged.

## Consequences

Hosts built without exceptions propagate a failure in one line.
`POSTPROJECT_TRY` and `POSTPROJECT_TRY_ASSIGN` become public names that must stay
stable within the integration-preview tier (ADR 0020). Existing C++ callers
rename `ok()`, `valueOr()`, and `takeError()`. The C++ examples, the
consumer tests, and the documentation use the macros wherever a function passes
failures on. A later move to C++23 can make `Result` an alias of
`std::expected<T, Error>`. That would need its own decision, because it changes
the exception thrown by `value()` and requires the macros to construct the
error through `std::unexpected`.
