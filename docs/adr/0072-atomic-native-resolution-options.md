# 0072: Native resolution options are created atomically

Status: accepted for development C ABI 53.

## Decision

Replace the five incremental C option mutators with one complete
`pp_resolution_config_t` input to `pp_resolution_options_create`. A null
configuration selects defaults. `PP_RESOLUTION_CONFIG_INIT` initializes the
ordinary C struct before the caller supplies overrides. The returned owner
is immutable and reusable; its single release function remains unchanged.

Copy and validate mappings, directory arrays, verification, bounds and the
cancellation flag in one operation. Borrow inputs only during creation.
Reject duplicate mappings, unknown verification modes, invalid bounds,
null/nonzero array pairs and counts above 100000 before exposing an owner.
Every failure clears the output. Retain the token's shared flag independently
of its handle. Replacing options requires creating a new valid owner first.

C++ `ResolutionOptions::create(config)` and Python's keyword path now call
this constructor directly; remove their private incremental helpers. Python
releases options deterministically after resolution, including failures.
ADR 0069's checked C++ contract remains unchanged.

## Migration and standards impact

Initialize a config, populate its fields, then check one creation result.
The changed creation signature and removed exports require ABI/soname 53.
The two input layouts bring the total to 38; there are 345 exports.
Bindings built for ABI 52 reject the new library before declaring functions.

Reviewed against the contributor standards policy: search policy, domain
meaning, schema 27, CLI JSON and exchange protocol 1 are unchanged. Mappings
remain local and resolution stays read-only; no standards mapping changes.
