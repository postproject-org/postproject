# ADR 0042: Semantic optimistic write conflicts

- Status: Accepted
- Date: 2026-09-30

## Context

Several processes may edit one local production. SQLite serializes physical
writes, but serialization alone does not reveal that a host made a decision
from stale production knowledge. Comparing only the production's latest
revision would also reject independent additive work. The conflict boundary is
a semantic fact, such as one resource's locator set or one object's metadata
property, rather than a database row or whole transaction.

The semantic revision journal describes changes for consumers. Making commit
cost depend on scanning its suffix would turn a notification contract into a
storage-engine index and make stale-write checks grow with history.

## Decision

A transaction may name an optional base revision. The base is the durable
revision on which the host's decisions were made; it is validated when the
transaction begins. It is not a lock or reservation. A transaction without a
base retains the existing serialized-write behavior.

The 0.7 development amendments are ADR 0045 (read-bound decisions/receipts)
and ADR 0052 (mergeable appends, property-version updates and mandatory bases
for metadata replacement/removal). The original release decision below
remains historical context.

Schema 17 adds a private conflict-version table mapping an encoded semantic key
to the last revision that changed it. A migration baseline records the newest
pre-schema-17 revision. When an old production is first opened, a base older
than that baseline is conservatively treated as conflicting for a touched key
that has no derived version yet. This can reject a stale edit unnecessarily,
but cannot permit a lost update. Normal schema-17 commits do not scan revision
history.

Mutations record the non-mergeable keys they touch while staging. At commit,
inside the same SQLite write transaction, storage:

1. compares each touched key's last sequence with the base sequence;
2. rolls back the entire transaction if one is newer;
3. otherwise appends the normal semantic revision;
4. updates every touched key to that new revision; and
5. commits all domain rows, events, and conflict versions atomically.

Checks and updates are proportional to the number of distinct touched keys.
On an optimistic conflict, commit rolls back and closes the transaction. The
caller retains structured conflict details, re-reads through the production,
and begins a new transaction for any explicit retry.

The public conflict key variants are:

- locator set for one resource;
- one metadata property on one object;
- dependency set for one representation;
- one media root;
- one exact external-identifier attachment on one object;
- one resource-fingerprint algorithm/version on one resource; and
- one representation-fingerprint algorithm/version on one representation.

A structured conflict reports that key, the supplied base revision and
sequence, and the revision and sequence that superseded it. Human-readable
error wording is diagnostic only.

## Mutation classification

Every public transaction mutation has one policy. `conflict` means the named
key is checked only when a base was supplied; `merge` means independent stale
transactions may commit; `existing-guard` names a stronger existing invariant.

| Mutation | Classification |
|---|---|
| set revision context | not-applicable |
| import original asset aggregate | merge (new identities) |
| add representation and new resources | merge (new identities) |
| add or confirm locator | conflict(locator set by resource) |
| retire locator | conflict(locator set by resource) |
| add media root | merge; existing-guard(root ID/name uniqueness) |
| enable, disable, or remove media root | conflict(media root by ID) |
| attach or remove external identifier | conflict(exact attachment); existing-guard(exact uniqueness/existence) |
| append metadata value | merge |
| replace or remove metadata property | conflict(object and property) |
| create activity and edges | merge (new activity identity) |
| replace dependency set | conflict(source representation) |
| record resource file facts | merge (non-identity observation filter) |
| record resource fingerprint | conflict(resource, algorithm, version) |
| record representation fingerprint | conflict(representation, algorithm, version) |
| request job | merge (new job identity) |
| claim, renew, release, fail, complete, or cancel job | existing-guard(job state, claim token, and lease) |
| commit or roll back transaction | not-applicable |

The event classification is likewise exhaustive:

| Revision event type | Classification |
|---|---|
| `asset_imported` | merge |
| `representation_added` | merge |
| `resource_added` | merge |
| `representation_resource_added` | merge |
| `locator_added` | conflict(locator set) |
| `locator_retired` | conflict(locator set) |
| `media_root_added` | merge with uniqueness guard |
| `media_root_enabled_changed` | conflict(media root) |
| `media_root_removed` | conflict(media root) |
| `external_identifier_added` | conflict(exact attachment) with uniqueness guard |
| `external_identifier_removed` | conflict(exact attachment) with existence guard |
| `metadata_added_or_replaced` | operation-dependent: append merges; replace conflicts |
| `metadata_removed` | conflict(metadata property) |
| `activity_created` | merge |
| `activity_input_added` | merge as part of new activity |
| `activity_output_added` | merge as part of new activity |
| `resource_fingerprint_observed` | conflict(resource fingerprint domain) |
| `representation_fingerprint_observed` | conflict(representation fingerprint domain) |
| `dependency_set_recorded` | conflict(dependency set) |
| `job_requested` | merge |
| `job_claimed` | existing job guard |
| `job_claim_renewed` | existing job guard |
| `job_claim_released` | existing job guard |
| `job_succeeded` | existing job guard |
| `job_failed` | existing job guard |
| `job_cancelled` | existing job guard |

The core event-type catalog exposes this classification through an exhaustive
match. Adding an event variant therefore fails compilation until its
concurrency policy is selected. Mutations without their own event remain
listed in the public transaction table above and in storage tests.

## Job concurrency

Job state, token, and lease checks remain authoritative. Generic base-revision
keys do not duplicate or reinterpret expired, lost, or invalid claims. Job
completion's new output and activity use new identities and remain additive.

## Origin identity

Revision origin remains application identity: application name plus optional
version and URI. It does not identify a person, workstation account, or
authenticated principal. Maintained integrations set it on every revision they
commit so conflict and change UI can explain which application wrote a change.

## Alternatives considered

- Comparing only the latest production revision was rejected because unrelated
  additive work would conflict.
- Scanning revision events after the base was rejected because commit cost
  would grow with history and journal encoding would become a lock protocol.
- Last-writer-wins was rejected because it silently discards a host decision.
- Automatic retry was rejected because only the host or user can reconsider a
  semantic decision using the new state.
- Treating job claims as generic optimistic conflicts was rejected because the
  token and lease protocol is more precise.
- Keeping a conflicted SQLite transaction open was rejected because its stale
  snapshot cannot support the required re-read and it would retain a writer
  lock.

## Standards impact

No external media, metadata, identifier, provenance, or interchange standard
defines this local optimistic-concurrency protocol. Vocabulary identifiers,
property names, external identifiers, and fingerprint domains remain opaque
and exact. The feature adds no normative mapping or normalization.

## Migration implications

Opening schema 16 creates the conflict-version and migration-baseline tables
and advances the production to schema 17. Existing semantic rows and revision
events are unchanged. Older builds reject schema 17 as newer than supported.
The additive public transaction and structured-error operations increase the C
ABI version when projected through C, C++, and Python.

## Consequences

Independent work from one base can commit, while stale writes to one semantic
fact fail atomically with machine-readable detail. Hosts must implement an
explicit re-read and retry flow. A base older than a schema-17 migration may
receive a conservative false-positive conflict for a key not changed since
that base; current productions have exact per-key versions after their first
schema-17 change.
