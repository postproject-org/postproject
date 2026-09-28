# ADR 0035: Reporting the outcome of a content observation

- Status: Accepted
- Date: 2026-09-28

## Context

ADR 0029 lets a native host observe a resource's present content. Storage
already treats an identical value as a no-op: an unchanged resource fingerprint
records nothing and creates no revision. The observation did not say which case
occurred.

Artifact evaluation reads recorded knowledge only (ADR 0022). A host that wants
to know whether an artifact is stale *now* must first bring the knowledge of its
sources up to date, because another application may have replaced a file. The
Kdenlive pilot did this on every project open and before every proxy render. It
verified the source, opened a write transaction only when the content differed,
observed it, and then evaluated, because it could not tell from the observation
alone whether anything changed. Other hosts face the same question:

- an editor opening a project whose media another application regraded;
- a render or transcode worker deciding whether its input changed since its
  last output;
- an asset manager or OpenAssetIO host refreshing what it shows the user.

## Decision

Observing content reports its outcome on every surface.

- **Unchanged:** the content matches a stored fingerprint in a domain
  PostProject computes. No resource fingerprint is recorded. A representation
  awaiting recomputation is still completed, which is recorded as its own
  observation.
- **Changed:** the content differs, and the new value is recorded as before.
- **First:** no fingerprint in a domain PostProject computes was stored, for
  example when a host recorded only its own hash. The value is recorded.

The comparison is the one verification performs (ADR 0029): the same domains,
and the stored values including observations staged earlier in the transaction.
The operations become:

- Rust: `ContentObservation::outcome`, returning `ContentObservationOutcome`.
- C: a required `pp_content_observation_t` output of
  `pp_transaction_observe_resource_content`, with `PP_OBSERVATION_UNCHANGED`,
  `PP_OBSERVATION_CHANGED`, and `PP_OBSERVATION_FIRST`. This bumps the C ABI to
  version 33.
- C++: `Transaction::observeResourceContent` returns
  `Result<ContentObservationOutcome>`.
- Python: `Transaction.observe_resource_content` returns a
  `ContentObservationOutcome`.
- CLI: `media fingerprint` prints the outcome, and its JSON output carries an
  `outcome` field.

The recommended sequence for a host is to observe, then evaluate. Verification
stays a separate read-only operation for callers that must not write.

## Alternatives considered

- **A separate "observe if changed" operation.** Observation already records
  nothing for unchanged content, so a second operation would differ only in
  its result. Reporting the outcome from the existing operation keeps one way to
  do it.
- **Evaluation that reads files.** Evaluation would become slow, depend on the
  machine it runs on, and give different answers on different machines. ADR 0022
  keeps it on recorded knowledge deliberately.
- **An optional output.** Every other result in the C ABI is required. Callers
  that do not need the outcome ignore it.

## Standards impact

None. The fingerprint domains are PostProject-defined (ADR 0015, ADR 0021).

## Consequences

A host keeps its knowledge current with one call per source, and learns from
the same call whether the source changed. Existing C callers must pass the new
output, which is a source-incompatible change covered by the ABI bump.
