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
retaining their exact bytes, identities and digests. Metadata/media replay uses
that storage in one writer transaction, with explicit receiver budgets and
original event cross-checks. Genesis initialization creates an empty passive
file with the source header and anchor. Production-metadata checkpoints retain
current assertions, original history, semantic guards and complete post-floor
records. Other domain sections and earlier development fragments remain unsupported.

Import uses an owned private directory on the destination filesystem, one SQL
transaction and explicit byte, disk and decoding-work budgets. The production
header becomes durable only with the fully validated passive state. Close SQLite
before exclusive promotion: Linux/macOS use `rustix`'s atomic rename with
`NOREPLACE`, failing on unsupported filesystems; Windows uses `tempfile`'s
`MoveFileExW` route without replacement. No link/unlink fallback is used on Unix.
These dependencies remain in the SQLite adapter; core gains no dependency.
Current assertions are checked against retained authored effects. A migration
floor may retain an unknown earlier property prefix, without inventing evidence.
See the upstream [rename API](https://docs.rs/rustix/1.1.5/rustix/fs/fn.renameat_with.html)
and [Windows implementation](https://github.com/Stebalien/tempfile/blob/v3.27.0/src/file/imp/windows.rs).

Private imports record an ownership marker and seal the closed database plus
that marker with a completion checksum. Restart discards an owned unsealed
directory or promotes an unchanged sealed stage. Unknown files, symlinks,
changed seals and existing destinations reject without removal. These private
checks establish neither source authentication nor portable authority state.

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

Media aggregate bodies use scalar headers and individual member, exception,
fingerprint and locator continuations. Existing core structure constructors check
membership/range invariants. Creation decoding retains bounded resource-identity
sets and yields evidence/locators individually, checking exact totals and complete
coverage before completion. Its facts remain provisional until the enclosing
transaction validates and commits. Standalone codec support does not establish
whole-production checkpoint coverage. Native capture and passive replay now
adopt complete original/representation creation, roots, locators and resource
file-fact and standalone fingerprint updates. Capture retains prepared values at their authored operation;
it never reconstructs creation from the final transaction state. A media staging
error rolls back its rows and queued facts while preserving unrelated edits.
Replay checks multiple original observations per creation effect in their source
order. Deferred references remain private until complete aggregate validation.
Fingerprint transitions retain previous evidence, exact archive order and every
affected recomputation marker. Clearing unchanged dirty evidence preserves its
original observation boundary; replay checks the recorded dependency invalidation.

Record manifests name the required domain codecs in a unique lexical set, covered
by their integrity commitment. Metadata-only records retain their earlier bytes;
media records add `media.v1`. Receivers validate the complete body before commit.
Legacy metadata evidence is encoded incrementally without a whole replacement
document allocation or a proposal-sized cap on native replacements.

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
