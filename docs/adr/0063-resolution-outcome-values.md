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

This changes no schema, persisted identity, locator encoding or standards
mapping. Candidate URIs and authored evidence retain their existing meaning.
