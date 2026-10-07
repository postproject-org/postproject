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

## Public values and standards impact

Use the case type to inspect its payload. Python reproducibility issues expose
their derived `kind` for display and use `isinstance` to narrow a payload.
The issue alternatives are `ReproducibilityProducerMissing`,
`ReproducibilityProducerAmbiguous`, `ReproducibilityToolMissing`,
`ReproducibilityParametersMissing` or `ReproducibilityInputMissing`.
C++ uses a `std::variant` of those case types; inspect it with `std::get_if`.
`artifactReproducibilityIssueKind` returns its display category. Ambiguous
producer counts use `ReproducibilityProducerAmbiguous::create` and
`activityCount()`; the other cases are ordinary aggregates of applicable IDs.
Python artifact reasons return the matching `Artifact…` record;
`isinstance(reason, ArtifactFingerprintChanged)` exposes required old/current
bytes and their domain. Dependency-path changes carry the direct input and
captured path, without a fabricated zero subject ID. Unrelated constructor
arguments are rejected. The display `kind` derives from the case type.
C++ `ArtifactReasonValue::create` checks a payload variant and exposes it
through a const `value()` view; copied values own their paths and evidence.
`ArtifactReason` is that checked value; `kind()` is its derived display tag.
Moved sources support destruction or reassignment. Input case records remain
unvalidated until the factory accepts them.

No schema, persisted identity or external standards mapping changes. These
projections preserve the existing recorded provenance and dependency semantics.
