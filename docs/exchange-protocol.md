# Portable exchange protocol

This development specification describes protocol major 1. Implementation and
cross-surface qualification are incomplete; the released 0.7 SDK does not offer
these workflows. See [ADR 0065](adr/0065-portable-knowledge-exchange.md).

## Scope and lifecycle

An authority has a production UUID and persistent history-generation UUID. A
mirror has those source identities and a separate local instance UUID. A base is
the scoped revision UUID/sequence, or explicit genesis with no UUID and sequence
zero. A replay position adds the committed digest. A mutation base is not a replay
cursor or a complete serializable read set.

A proposal includes `kind`, `version`, `required_features`, production/history,
client/request IDs, nullable base, origin/message, ordered commands and
`extensions`. New entity IDs are prepared once before submission. Commands
cannot choose authority time, revisions, conflict versions or privileged state.
Missing bases are allowed only by the existing additive/state-guarded contracts.

The submission codec currently supports `metadata.append`, `metadata.replace`,
`metadata.remove`, `root.add`, `root.set-enabled`, `root.remove`, `locator.add`,
`locator.retire`, `identifier.add`, `identifier.remove` and
`resource.observe-file-facts`, `media.import-original`, `representation.add`,
`resource.observe-fingerprint`, `representation.observe-fingerprint`,
`dependency.observe-set` and `activity.create`.
Prepared aggregates contain scalar asset/representation/resource headers,
complete structure frames, locators and fingerprint algorithm/version/bytes.
Their fingerprints cannot supply observation revisions; storage assigns these.
Asset creation and filesystem times remain prepared source evidence.
Dependency intent retains complete ordered occurrences, including an empty set;
it supplies no extraction state or recording revision. Activity intent supplies
timing, tool/agent attribution and canonical edges, with no captured snapshots.
Storage captures fingerprint/dependency evidence at publication. Activity
parameters and identifiers use subsequent metadata/identifier commands.
Nested media facts retain exact checked values;
retirement supplies only the locator ID. Measured file facts carry no assigned
revision. Destructive operations keep their native base requirements.
Proposal requirements are the exact unique lexical set of command codecs
(`dependencies.v1`, `media.v1`, `metadata.v1`, `provenance.v1` as needed);
an empty proposal retains `metadata.v1`.
Missing, redundant or reordered declarations reject. Worker commands are not
yet implemented in submission.

An outcome contains the original scoped request identity and either its own
accepted receipt (nullable new revision) or a structured terminal domain
rejection. Recover a lost reply by looking up or resubmitting the same identity.
Changed intent needs a new request. Public lookup never delivers credentials.
Claim activation occurs at commit; private delivery happens separately. Lost
credential delivery requires expiry or coordinator cancellation, without reissue.

A committed record contains scope, predecessor position/digest, assigned
revision/transaction/time/context, ordered complete effects, public events and
extensions. Native writes use their original transaction identity and need no
invented client request. Historical effects preserve intermediate observations
that remain publicly meaningful, rather than only the final state.
Accepted submission extensions survive in the original record manifest, replay
and retained checkpoint records; they remain part of request equality.

At migration, retain old revisions and create a persistent replay-floor anchor.
Its digest uses the `anchor` domain over canonical production/history/floor.
Requests before that floor report `history_gap` and require a checkpoint. Export
returns complete unfiltered records through a fixed advertised head. No-change
outcomes create no record. Empty history and missing history are distinct.

## Exact values and canonical bytes

Documents are UTF-8 JSON objects without a BOM. Duplicate keys, invalid Unicode
scalars and trailing data reject. All integers, including version/count/unit
fields, are canonical decimal **strings**: `0`, positive digits without leading
zeros, or `-` followed by a nonzero magnitude. JSON number tokens reject, including
in extensions. Booleans remain JSON booleans and never stand in for integers.

UUIDs are lowercase hyphenated text. Signed/unsigned widths and storage ranges
are checked for the field's domain. Decimals contain `coefficient` and `scale`;
rationals/rates contain `numerator` and `denominator`. Decode through existing
checked constructors and encode the resulting domain representation. Binary
values use RFC 4648 standard base64 with padding and zero unused pad bits;
whitespace and alternate alphabets reject. Times name their microsecond unit.

Metadata uses explicit tags for string, language string, i64, u64, decimal,
boolean, timestamp, URI, bytes, rational, list, ordered struct fields and typed
reference. Ordered/repeated values and struct fields are arrays. Exact text,
identifiers and URI spelling are retained after existing domain checks.

Canonical objects sort keys by their UTF-8 bytes; arrays keep their order. There
is no insignificant whitespace. Encode Unicode scalars literally in UTF-8;
escape quote/backslash and use `\b`, `\t`, `\n`, `\f`, `\r` for those controls.
Other U+0000–U+001F controls use lowercase `\u00xx`. Slash, U+2028 and U+2029 are
literal. These rules apply recursively, including extensions. This profile does
not claim RFC 8785/JCS conformance.

The digest is 32-byte BLAKE3 in derive-key mode over canonical validated bytes.
Contexts are exactly `postproject.exchange.v1.` followed by `request`, `record`,
`chunk`, `manifest`, `anchor` or `state`. Wire digests are 64 lowercase hex digits.
The digest-bearing object's own `digest` field is excluded; every other field,
including required features, predecessor and extensions, is included. A record
manifest binds the original revision and predecessor, effect/event counts and a
chunk summary: exact count, reconstructed byte count and final chained digest.
Each chunk binds its scope, revision, index, preceding chunk digest and contents.
The first chunk has a null predecessor. This keeps record headers bounded even
for very large native commits. A checkpoint manifest has a
separate identity/digest from its stable source continuation anchor.

Record requirements are unique and lexically ordered: `dependencies.v1`,
`jobs.v1`, `media.v1`, `metadata.v1`, `provenance.v1` and `record-chunks.v1` are recognized.
Every record requires chunk framing and at
least one domain codec. Existing metadata manifests keep their exact two-feature
encoding. A receiver must reject an unsupported body before publishing state;
understanding the manifest alone does not establish media replay support.

Unknown critical fields/tags/required features reject atomically. Extensions are
a bounded object whose keys are namespaced identifiers containing `:`; values
follow the same strict JSON profile. Preserve extensions in equality/outcomes
and records. Unknown metadata vocabularies are supported ordinary domain facts.

## Media body facts

Standalone media codecs separate scalar `asset.header`, `resource.header` and
`representation.header` facts from aggregate collections. `structure.header`
declares a single resource, compact image sequence, ordered parts or package.
Ordered `structure.member` and sorted `structure.exception` frames follow its
exact totals. Existing core limits remain 100,000 members or sparse exceptions;
these are independent of the proposal-command cap. Sequence naming belongs to
each `locator.fact`, rather than the content descriptor.

`root.fact` retains configured roots and their historical legacy URI, without
local directory mappings. `identifier.attachment` retains asset, representation,
resource and activity targets,
schemes, values and qualifiers. `identifier.added` and `identifier.removed` retain
authored transitions, including same-revision removal/re-addition and distinct
optional qualifiers. `fingerprint.snapshot` preserves original bytes and nullable
observation sequences. `fingerprint.observation` distinguishes
current evidence from ordered superseded values, including changes within the
same revision. `fingerprint.recomputation` records the original marking boundary.
`fingerprint.change` retains previous/current evidence, original archive position,
marker count, cleared dirty evidence and dependency invalidation. Its following
markers are ordered by representation UUID. An unchanged dirty clear preserves
the current fingerprint's observation boundary. Replay checks prior evidence,
archive order and complete owner coverage without measuring media.
All existing observation kinds retain their original revision and position;
public job notifications contain only the job ID and operation kind.

`dependency.set` declares the original recording revision, extraction status and
exact occurrence count, including a complete empty observation. Individual
`dependency.occurrence` frames retain authored order, repeated occurrences,
optional source resources, floating or pinned targets, recorded resolution,
requiredness and exact reference text. Native replacement emits current status;
replay validates references and the original revision without extracting content.
It rejects equivalent current replacements that would have been native no-ops.

`activity.header` retains assigned identity, exact kind, optional authored times,
checked tool/agent attribution and native input/output counts. Empty input sets
are valid; at least one output is required. Ordered `activity.edge` headers carry
representation/role, the original snapshot boundary, fingerprint counts and an
explicit dependency-snapshot marker. Absent legacy evidence differs from an
explicitly empty observation. Input paths follow their edge fingerprints:
`activity.dependency-path` declares status, subject, segment and fingerprint
counts; `activity.dependency-segment` retains both path and authored dependency
positions. Individual subject fingerprints follow the segments.

Native capture streams immutable staging-time rows inside the original commit,
including evidence predating later changes in that same edit. Replay consumes
those facts and checks counts, ordering, references, cycles and complete required
path coverage against the applied authored prefix. It generates no replacement
snapshots and does no media I/O. Existing 64-segment and 1,000-visited-representation
capture limits retain their explicit truncation statuses.

`job.header` retains the stable request ID, exact work kind, input count, requested
asset/representation role/root and one of the five observed states. Following
`job.input` frames preserve canonical input UUID order and original positions,
within the native 100,000-input bound. A request effect starts in requested state.
`job.transition` retains the original operation, before/after state, nullable
authority time and claim/completion input decision sequence. Cancellation has no
authority time; other transitions retain it exactly. Claim/renew durations retain
the native whole-microsecond range from 1 µs through 24 hours. An input boundary of zero
identifies a genesis decision. These fields describe the source decision and
grant no authority to the receiver.

Claimed detail preserves tool/agent attribution and expiry; succeeded detail
preserves activity/output IDs; failed detail preserves the exact diagnostic.
Bearer claim IDs are forbidden. Replay checks prior state, lifecycle constraints,
input boundaries and completion's requested output/producing activity. It never
consults its clock or authorizes work. Schema 25 materializes claimed mirrors
with a derived inert marker and NULL credential, preserving their native read
projection. Neither the marker nor private authority clock enters the wire.

`original.creation` retains the assigned asset header and is followed by its
complete original representation. `representation.creation` declares scalar ownership plus resource, locator and
representation-fingerprint counts. Continuations contain representation evidence,
the complete structure, each `resource.creation` header and its evidence, then
locators. Initial fingerprints retain the original observation revision. Decode
yields provisional facts individually and verifies exact ownership, order, totals
and resource/location coverage before completion. The enclosing transaction must
discard every staged prefix on failure. Rust native capture and passive replay
now adopt complete original/representation creation, roots, locators, identifiers and changed
resource file facts. Root/locator effects retain additions and intermediate
enabled/removal states; file-fact effects retain authored sizes/times. Standalone
fingerprint updates retain exact historical boundaries and recomputation facts.
Whole-production checkpoint bodies remain incomplete.

## Bounds, checkpoints and replay

Defaults: 64 MiB per proposal, 1,000 proposal commands, 32 MiB per encoded chunk,
192 container levels and 1,000,000 JSON nodes per bounded document. Domain limits
remain authoritative, including metadata's 32 levels and 15 MiB aggregate value.
Callers may lower codec limits; exceeding them reports `limit_exceeded`.
Native transaction size has no proposal-command cap.
Legacy authored metadata evidence also streams one value at a time into 1 MiB
storage fragments, preserving the original canonical document bytes. A native
replacement may exceed the 64 MiB proposal limit without creating extra revisions.

Record chunks carry at most 16 MiB of raw bytes as padded base64, leaving room
for their envelope within the 32 MiB encoded limit. A fragment may split UTF-8
or a domain item; validate reconstructed domain items before replay. A logical
record digest covers its unsigned manifest in the `record` domain. Its manifest
digest covers that header plus `record_digest` in the `manifest` domain. Complete
chunk framing alone does not prove valid domain state.

The reconstructed record body is a sequence of frames: an unsigned 64-bit
big-endian byte length followed by exactly that many canonical UTF-8 JSON bytes.
Zero-length frames are invalid. Frames may span chunks. A `metadata.effect`
header declares the authored operation, target, property, optional assigned
append position and exact value count. Its ordered `metadata.value` frames follow;
empty replacements and removals carry none. Original `observation` frames follow
all effects, retaining their revision IDs and contiguous positions. Replay checks
the manifest's total effect/event counts and consumes the entire body.

The current Rust metadata/media receiver stages verified canonical chunks inside one
writer transaction, then decodes complete effects and checks original events.
It validates target existence and assigned append positions; updates semantic
property versions; and commits facts, original history and the source boundary
together. One creation effect may yield several original observations; replay
checks their exact original order independently of the effect count. Media
references are deferred inside private staging until the complete aggregate
validates. Input iterator errors cancel the transaction. Identical duplicates
still require the complete matching chunk chain, without applying effects again.
Default receiver budgets are 1 GiB of total encoded manifest/chunk bytes and the
per-document limits above. Callers may explicitly raise receiver budgets to
accept larger native records. These defaults do not cap native transactions.

Large logical records use a manifest and ordered bounded effect chunks. Large
aggregate/binary fields use declared ordered continuations with exact totals and
digest validation. No truncation or extra visible revisions is permitted. A
checkpoint streams sections through one retained view rather than assembling the
production in memory. Staging disk/work budgets and cancellation are explicit.

Checkpoint sections cover the production header, entities/content, locators and
roots, identifiers, ordered metadata, activities and snapshots, dependencies,
fingerprint history/recomputation, public jobs/input boundaries, revisions/events
and semantic conflict floors/versions. Derived indexes are rebuilt. SQLite row
IDs, local mappings/caches, worker secrets, operational clock and private dedup
bindings are excluded. Public ordering uses section-defined positions.

Checkpoint framing declares all 18 sections, including explicit empty ones, in
this order: production, assets, resources, representations, structures, roots,
locators, identifiers, metadata, activities, dependencies, fingerprints, jobs,
revisions, events, conflict versions, conflict floor and retained records.
Production and conflict floor each contain one item. Retained revision totals
match the source head sequence; record totals match head minus source replay
floor. A nonempty section declares item count and complete chunk-chain summary;
an empty section declares zero items and no chunks. Chunks bind source scope,
checkpoint identity, section, index, predecessor digest and exact payload bytes.
Rust export/import currently supports production metadata and its retained
history/guards/records. Metadata items are `metadata.assertion` frames with exact
target, property, position and typed value. `revision.observation` retains the
original context/time/transaction. `conflict.version` uses a semantic key;
`conflict.floor` retains the independent conflict migration baseline. Each
retained-record item starts with its original manifest, followed by exactly its
advertised record chunks. Frame and section boundaries must be fully consumed.
Other domain bodies, pre-floor development fragments and nonempty checkpoint
envelope extensions reject as unsupported; complete production import is pending.

Checkpoint receiver defaults are 1 GiB encoded transport, 2 GiB private staging
disk and 10,000,000 decoded frames, including nested record frames. Callers may
raise them explicitly. Import checks assertion order, original history, semantic
versions and the current values implied by retained effects. Earlier migration
prefixes remain baseline facts rather than invented historical edits.

Private staging ownership/completion checks support process restart: discard an
owned unsealed import or promote its unchanged sealed complete store. Unknown
files, symlinks, altered completion evidence and existing destinations reject.
Ownership markers and closed-file checksums remain local, without an authenticity
claim. Recovery runs after the importing process has stopped.

A complete manifest declares source scope, checkpoint identity, anchor, replay
floor and ordered section/chunk counts/digests, including empty sections. Publish
only after every chunk is durable. Import validates references, cycles, exact
completeness and exclusions into private staging; close SQLite/WAL before atomic
same-filesystem promotion to a **new** destination. Import never merges stores or
restores authority. Incomplete staging is invisible and owned cleanup is bounded.

Apply checks source scope, sequence, predecessor, features, chunks and structural
invariants before atomic visibility of facts/revision/events/applied position.
An exact applied duplicate is a no-op; changed identity/content at that sequence
reports `divergence`. Missing predecessors report `history_gap`. Replay does not
allocate revisions, measure media, execute work or authorize historical leases.
Ordinary reads may resolve differently because of explicit local availability.

## Failures and caller recipes

Required typed categories: `malformed`, `unsupported`, `limit_exceeded`,
`scope_mismatch`, `invalid_base`, `request_identity_mismatch`, `history_gap`,
`integrity`, `divergence`, `mirror_read_only`, and existing structured domain
conflicts. Storage failures retain their retryable/uncertain classification.
Diagnostics contain neither input documents nor worker secrets.

Build a proposal against an opened authority and optionally bind its detached
decision; submit once, retain its request IDs and inspect the original receipt.
Export a pinned checkpoint to a stream/path; validate/import into a new mirror.
Export changes after its scoped position; apply complete records and obtain the
durable position. Inspect role/scope, limits, required features, floor and head.

C uses checked operation-specific builder inputs and owning result/stream
handles; C++ uses RAII and Result/exception conventions. Python uses UUID hints,
typed command unions and ordinary exact values. Rust uses standard I/O/path
types. CLI `exchange` operations use proposal files and private token files/stdin.
The existing CLI format-1 envelope is independent of nested protocol version.
