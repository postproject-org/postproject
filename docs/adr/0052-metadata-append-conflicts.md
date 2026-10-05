# 0052: Mergeable metadata and destructive decisions

Status: accepted for development.

## Decision

Appending a metadata value is an independent additive operation. Concurrent
appends may merge; their order follows durable transaction order. Replacing
or removing the property is a decision about its complete ordered contents.
Such a decision must conflict if any writer changed that property after its
base, including an append.

Track keys whose versions change separately from keys a transaction guards.
An append advances the property's version without rejecting a stale additive
base. Replacement/removal both guard and advance that same key. Persist key
versions, assertions, events and the receipt in one atomic commit. Rejected
transactions roll back all their work and become terminal.

The distinction does not establish full read-set serializability. Other
semantic conflict families retain their current policy.

## Migration and standards impact

Previously an append did not advance the property version, allowing a stale
destructive edit to erase it. Such commits now return a structured metadata
property conflict. Reread and make a new decision before retrying.
Appends remain permitted without a base. Persisted UUIDs, metadata encoding,
C signatures and schema 17 are unchanged.

Reviewed against `docs/src/contributors/standards-policy.md`: no vocabulary,
cardinality, normalization or external metadata mapping changes.
