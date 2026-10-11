# 0069: Atomic C++ resolution configuration

Status: accepted for development; supersedes ADR 0048's C++ option setters.

## Decision

`ResolutionOptions::create(config)` checks a complete `ResolutionConfig` and
returns either a move-only native owner or an error. The config uses standard
containers and filesystem paths for logical-root mappings/search directories,
and ordinary fields for verification and traversal bounds. Defaults match
native resolution. A borrowed cancellation-token pointer is consumed only
during creation; the native owner retains the flag independently.

Remove public incremental setters. Construct another checked configuration
when a workflow changes verification or cancellation. Failed construction
destroys the temporary native owner, exposes no partial configuration and
leaves any existing options untouched. Default resolution needs no options
object. Result and exception styles share this factory; C++17 and builds
without exceptions remain supported.

Natron's presence-then-content check is a real reconfiguration caller. It
reuses ordinary config values and replaces options only after successful
validation. It does not postpone errors until resolution. Private helpers
reuse authoritative C validation; no new domain rules enter the wrapper.

## Migration and standards impact

Replace empty creation plus checked setters with config fields followed by
one checked factory. Native tests retain invalid bounds/mode/path cases,
moved-owner/token rejection and cancellation after token destruction.

Reviewed against the contributor standards policy: no identifier, locator,
storage, C ABI or portable encoding changes. UTF-8 filesystem paths cross
the existing C boundary. Historical 0.6.x promises and artifacts are unchanged.
