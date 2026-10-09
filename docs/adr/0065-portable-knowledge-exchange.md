# 0065: Portable knowledge exchange

Status: accepted for development; implementation and qualification are incomplete.

## Decision

One local authority accepts ordered semantic proposals. Ordinary native edits
continue to use domain operations. Both paths capture complete committed effects
inside their original transaction. Observation events remain a query projection;
they are insufficient for reconstruction.

A passive mirror retains the source production and history generation, with its
own local instance ID. It accepts validated checkpoints and contiguous effects
only. Ordinary edits, worker credentials and execution reject on a mirror.
Reading, evaluation and resolution with local mappings remain available.

Persist a history generation once on creation or migration. Migration anchors
replay at the existing head; earlier revisions remain observation history.
Checkpoint manifests and history continuation digests have separate identities.
Exporting another checkpoint does not change the continuation anchor.

Schema 23 starts complete record storage at its migration head. Earlier
development effect fragments remain partial evidence, without a replay claim.
The source generation and retained submission outcomes remain unchanged.
Schema 24 fragments canonical chunk envelopes below SQLite's value bound while
retaining their exact bytes, identities and digests.

Bind `(production, history, client, request)` to the complete normalized proposal
and a private credential binding. Persist accepted, no-change and terminal domain
rejections durably. Equivalent duplicates return the original outcome before
checking current state. Different intent with the same identity rejects. I/O,
contention and uncertain delivery are recoverable storage failures, not cached
domain rejections. No outcomes or records are pruned automatically.

The writer transaction commits domain facts, historical evidence, conflict
versions, revision/events, complete effects and an accepted outcome atomically.
Use a savepoint to discard a rejected proposal's staging before recording its
terminal rejection. A native commit cannot succeed if effect persistence fails.
No-change commits retain the existing receipt without a revision.

Checkpoints pin one coherent view and include all portable current and historical
facts. Import validates bounded chunks in a private staging store, then promotes
a closed complete passive store to a new destination. Applying a logical record
and its position is atomic even when the record has many chunks. Exact duplicates
are no-ops; gaps, altered duplicates and unsupported required effects reject.

Effects retain assigned IDs, times, ordered metadata, removals, fingerprints and
captured dependency/provenance evidence. Apply performs structural validation,
without media I/O, execution, new revision allocation, lease authorization or
clock decisions. Claimed jobs remain faithful inert observations. Bearer claim
IDs, tokens, clock authority and private outcome bindings are never exported.
An ordinary checkpoint cannot restore authority or resume a worker.

The backend-neutral `postproject-protocol` crate depends on core. SQLite and
adapters may depend on it; media remains independent. Core gains no codec or
framework dependency. The wire rules and failure contracts are specified in
[the protocol specification](../exchange-protocol.md).

## Compatibility and limits

Protocol major 1 is experimental and independent of package, ABI, schema and CLI
envelope versions. Existing read-session/query-cursor scopes remain unchanged.
Protocol bases and positions include source-history scope; matching UUIDs alone
cannot authorize rebinding to another authority.

Local file/pipe exchange establishes conformance without a network backend.
Digests establish integrity relative to an anchor, not authenticity. The system
requires one authority; a malicious fork or copied authority is not fenced by a
history UUID. Editable replicas, promotion, backup restoration and authentication
are outside this contract. Retained history/outcomes consume disk without pruning.

## Standards impact

Reviewed [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259), sections 4 and 7–9,
and [RFC 4648](https://www.rfc-editor.org/rfc/rfc4648), sections 3–4, on
2026-10-08. Require unique JSON keys, Unicode scalars and strict padded base64.
Canonical encoding is a PostProject profile, without a JCS conformance claim.
[BLAKE3's upstream vectors](https://github.com/BLAKE3-team/BLAKE3/blob/master/test_vectors/test_vectors.json)
provide independent hash checks. Existing identifiers and vocabulary values
retain their exact spelling; no new external mapping, normalization, cardinality
or provenance authenticity claim is introduced.
