# ADR 0034: A warning-free C++ header in host builds

- Status: Accepted
- Date: 2026-09-28

## Context

The C++17 wrapper is header-only. Every host compiles it with its own compiler,
language standard, optimization level, and warning flags. A warning the header
raises appears in every host's build log. A host that builds with warnings as
errors, as many distributions and CI setups do, cannot use the header until it
suppresses the warning.

The Kdenlive pilot, built with GCC 15.3 at `-O2`, reported
`-Wmaybe-uninitialized` inside `evaluateArtifact` for the optional fingerprint
values of an artifact reason. The values were always initialized, so the warning
was a false positive. PostProject's own checks had not seen it:

- CI builds the C++ consumers and every documentation example with
  `-Wall -Wextra -Wpedantic -Werror`, but only with the runner's default GCC.
- GCC 15 raises the warning only when the call is compiled in a small function
  of its own, with exceptions enabled, as a host adapter calls it. Inside the
  larger example programs it does not.

## Decision

The installed C++ header compiles without warnings in host builds.

- A function whose optimized form triggers a compiler false positive is
  restructured, not annotated with a diagnostic pragma. `evaluateArtifact` now
  builds each reason and then assigns its fingerprint values in place.
- `tools/check_cpp_header_warnings.py` compiles every C++ program in the
  repository against `include/`. Each program is compiled as C++17 and as
  C++20 at `-O2`, with `-Wall -Wextra -Wpedantic -Werror`, and only compiled,
  never linked. The programs are the documentation examples, the installed
  example, and the consumers in `tests/abi`. The no-exceptions consumer is
  compiled with `-fno-exceptions`.
- `tests/abi/cpp_isolated_calls.cpp` holds wrapper calls in isolated functions
  that once raised a warning.
- A CI job runs the check with the current GCC and Clang from an Arch Linux
  container, in addition to the existing consumer builds with the runner's
  compilers.

## Alternatives considered

- **Suppress the warning with `#pragma GCC diagnostic`.** Suppression hides
  real warnings in the same function and spreads compiler-specific pragmas
  through the header.
- **Tell hosts to disable the warning.** This moves PostProject's problem into
  every host's build files.
- **Compile the examples with more compilers only.** The warning depended on
  the calling context, so the examples alone did not reproduce it.

## Standards impact

None.

## Consequences

A new compiler release can fail the check before any host sees its warning.
The check takes a few minutes of compile time, because every program is
compiled four times per compiler. It uses no Cargo and no native library.
