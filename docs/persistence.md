# Persistence

Each production is one SQLite database. The backend enables foreign keys and disables
trusted-schema features on every connection. A five-second busy timeout turns
brief lock contention into bounded waiting rather than an immediate failure.
SQLite's per-connection value-length limit is reduced to 16 MiB before migrations
or queries run. This bounds allocations for strings, blobs, and result rows read
from an untrusted production file while leaving ample room for production metadata.

## Schema version 14

The current development schema stores a singleton production record plus assets,
representations, content structures, resources, memberships, locators, typed
fingerprints, logically named media roots, metadata assertions, and external
identifiers, provenance activities and edge snapshots, dependency observations,
the revision journal, and durable jobs. Machine-local root mappings are
intentionally not production rows; schema 6 retains migrated absolute URIs only
as transitional legacy fallbacks.
Image-sequence descriptors and their known missing frames are stored compactly;
a regular sequence does not require one resource row per frame. Public
identities are 16-byte UUID values; SQLite row numbers are never exposed.

Constraints enforce ID lengths, enumeration ranges, bounded text and blobs,
non-empty fingerprint values, and referential integrity. Indexes support
assets by creation order, representations by asset, resources by
representation, locators by resource, external identifiers by target and exact
scheme/value, metadata by target, property, or property and exact encoded value,
enabled media roots by priority, provenance edges by representation, revision
events by target, and jobs by state and kind or by kind alone. Each index used
by a paginated domain query ends in the stable key used for cursor continuation.

Some domain queries select by a fact that spans tables, which no single SQLite
index can express. Schema 12 therefore keeps three derived query-support tables:
required memberships whose resource has no locator, representations reachable
through a locator recorded under each logical root, and each activity output
keyed by its activity kind and tool identity. Triggers maintain them on every
insert, update, and delete of the authoritative rows, and the migration
backfills them from existing knowledge. They hold no knowledge of their own, are
never written directly, and let unresolved-media, media-root, and
activity-output pages read rows proportional to the page even when matches are
sparse. Schema 13 adds a fourth, keyed by revision event kind and revision
sequence, which a trigger fills from every journaled event. Event-type-filtered
revision pages read at most one page of keys per requested kind, however long
the run of unrelated revisions after the cursor. Schema 14 indexes jobs by
their completion activity, so a regeneration plan finds the job that produced an
artifact without scanning the job table.

External identifiers and metadata assertions use polymorphic typed targets.
Jobs are valid targets alongside production, asset, representation, resource,
and activity objects. SQLite triggers clean up attachments because one target
column cannot carry foreign keys to several domain tables.

## Migrations and durability

`PRAGMA user_version` identifies the current schema, while `schema_migrations`
records every applied numbered migration and its timestamp. Each migration runs
inside an immediate SQLite transaction. A failed statement therefore leaves both
the prior schema and version intact. Opening a newer unsupported schema fails
without modifying it. Every numbered schema from the published production-file
history migrates forward in order; migrations preserve absent historical
observations rather than inventing evidence.

Production creation reserves a new file without overwriting any existing path, runs
migrations, then inserts production identity and metadata in one transaction. Normal
SQLite transaction durability applies. Backup tooling should copy a closed production
or use SQLite's online backup API once that API is exposed; copying only the main
file while a production is open may omit WAL state.

The backend currently builds a bundled SQLite for reproducible developer and CI
builds. SQLite errors are wrapped as domain storage or migration errors rather than
becoming part of the public contract.

## Domain transactions

Media imports insert the asset, representation, content structure, resources,
typed fingerprints, memberships, and initial locators inside one explicit
deferred SQLite transaction. Media-root creation, enablement and removal;
locator retirement; metadata assertions; and external-identifier
attachments/removals; dependency observations; fingerprint observations; and
job transitions participate in the same transaction boundary. Job completion
also binds staged output media and its provenance activity before commit. Commit
and rollback close the transaction; repeated close attempts return a conflict.
Dropping an open transaction uses SQLite rollback semantics, so partially
staged changes never become visible.

## Revision waits

A revision waiter opens its own connection to the production file, marks it
query-only, and verifies that the file still holds the production it was
created from. It never uses the production's connection, so blocking does not
hold the production. Commits through the same production wake its waiters
through an in-process signal. Commits by other processes or handles are found
by polling `PRAGMA data_version` on the waiter connection, starting at 5 ms and
backing off to 100 ms while nothing changes; the journal is read only when the
data version moves. The waiter connection uses a 20 ms busy timeout and treats a
busy file as unchanged until the next poll, so a long writer cannot hold a
waiter past its timeout. Dropping or closing the production wakes its waiters
with a closed outcome. ADR 0027 records the design.
