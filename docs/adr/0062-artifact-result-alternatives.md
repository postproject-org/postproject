# 0062: Artifact result alternatives

Status: accepted for the 0.7 development SDK.

## Decision

Artifact reasons and reproducibility issues carry one closed alternative.
Each case contains its applicable identities, counts, path or fingerprint
evidence; unrelated fields cannot be independently attached to it. Rust retains
its enums, C uses checked projections, C++ variants and Python frozen records
with union annotations. Required unknown kinds fail explicitly.

These are copied observations, not authority or requests to change knowledge.
Construction checks payload constraints; operations still establish existence
and production scope. Optional fingerprint evidence remains optional within the
cases that support it. Bounded paths and opaque fingerprint bytes retain their
meaning and ownership.

## Migration and standards impact

Use the case type to inspect its payload. Python reproducibility issues expose
their derived `kind` for display and use `isinstance` to narrow a payload.
Replace flat issue construction with `ReproducibilityProducerMissing`,
`ReproducibilityProducerAmbiguous`, `ReproducibilityToolMissing`,
`ReproducibilityParametersMissing` or `ReproducibilityInputMissing`.

No schema, persisted identity or external standards mapping changes. These
projections preserve the existing recorded provenance and dependency semantics.
