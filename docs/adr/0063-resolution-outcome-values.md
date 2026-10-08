# 0063: Resolution outcome values

Status: accepted for the 0.7 development SDK.

## Decision

A resource resolution contains one outcome: known locator, exact candidate,
probable candidate, offline, ambiguous or failure. Successful outcomes contain
one candidate; ambiguity owns at least two; offline and failure contain none.
Rust retains its checked domain factory. C exposes checked state projections,
C++ a checked variant and Python frozen case records with a union annotation.
State and candidate access derive from that outcome.

Candidates own their evidence and retain confidence in 0..10000 basis points.
Copied binding values check payload shape and bounds; URI normalization, root
names, existence and confirmation remain authoritative domain operations.
Resolution remains read-only and never chooses among ambiguous candidates.

## Public values and standards impact

Python callers can inspect `resource.outcome` with `isinstance` and continue
using derived `resource.state` and `resource.candidates` for display.
An ambiguous outcome copies its candidate collection; candidate evidence is
also copied. Required unknown native states return `UnsupportedError`.
Native state/count inconsistencies return `InternalError`.
C++ `ResolutionOutcome::create` checks case cardinality and candidate payloads.
Its const `value()` and `candidates()` borrow owned data; `state()` derives the
category. `ResourceResolution` forwards the latter accessors. Input case
records are unvalidated requests. Copy assignment preserves the old value if
allocation fails; moved sources support destruction or reassignment.

In C ABI 51, `pp_resolution_set_get_resource_state` selects the expected
state for `pp_resolution_set_get_resource`. Wrong-state access clears all
outputs and returns `InvalidArgument`; candidate indices remain checked by
their accessor. Borrowed values last until the resolution set is released.

Availability diagnostics likewise carry one detail case: offline, ambiguous,
resource error or missing frames. Only `MissingSequenceFrames` owns frame
numbers, with a nonempty bounded canonical list. Python's `detail` union keeps
`kind` and `frames` as derived display properties. C++
`AvailabilityIssueDetail::create` checks a private variant; `value()` borrows
its case. `AvailabilityIssue::kind()` and `frames()` derive from that detail.
Its copy/move policy matches the resolution outcome.

This changes no schema, persisted identity, locator encoding or standards
mapping. Candidate URIs and authored evidence retain their existing meaning.
