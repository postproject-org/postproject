# ADR 0026: Domain query cursors

- Status: Accepted
- Date: 2026-09-25

## Context

Production-sized enumerations cannot safely return every matching row. The
existing direct-dependent and job reads do exactly that, while ADR 0024 defers
paginated direct and transitive dependency queries to the common query cursor.
Offset pagination would become progressively more expensive and can repeat or
skip rows when concurrent commits move offsets.

The cursor contract must work through Rust, C, C++, Python, and the CLI without
exposing a backend query language or SQLite row identities. Dependency graphs
may contain cycles, so a page-size limit alone does not bound traversal.

## Decision

Named domain queries use a common request and result shape:

- a page size from 1 through 1,000;
- an optional opaque continuation token returned by the preceding page;
- results in a documented stable key order; and
- an optional next token only when another page exists.

Tokens are versioned, query-scoped, and bound to every parameter that affects
membership or ordering. Reusing a token with another query, root object,
filter, or traversal bound is an invalid argument. Tokens are bounded UTF-8
values that callers may copy or serialize, but their contents are not an API
and may change before 1.0. They are continuations, not production identities,
revision cursors, authentication credentials, or durable snapshots.

SQLite adapters implement tokens as keyset positions. Queries compare the
stable domain key after the token and fetch one extra row to decide whether to
return a continuation. They never use `OFFSET` and never expose a SQLite row
number.

Dependency and dependent queries additionally require a maximum depth from 1
through 64 and a maximum of 1 through 1,000 traversed representations. Their
page reports explicit traversal truncation independently of the continuation
token. Cycles are suppressed within the bounded traversal. A depth of one is a
direct query; larger depths include the transitive closure. Forward results
name asset or representation targets and reverse results name dependent
representations, each with its shortest observed depth.

Job enumeration adopts the same cursor immediately because it is already an
unbounded whole-set read. It accepts optional exact state and kind filters and
orders by job ID. The existing job state/kind index backs the combined filter;
additional indexes are added only where an accepted query shape needs them.

Asset, per-asset representation, structural resource, and per-resource locator
enumerations use the same page contract. Representation pages load structures
and fingerprints with set-oriented reads rather than one query per result.
Metadata queries accept an optional exact deterministic-encoding predicate for
scalar values. Activity-output queries use exact activity-kind or complete tool
identity predicates. Producing and consuming activities are selected in SQL.
Provenance traversal reports shortest depth with the same explicit depth and
visited-representation bounds as dependency traversal.

The knowledge-only media-root query requires durable knowledge of which logical
root produced a confirmed locator. A locator may therefore record an optional
logical root name in schema 11. Existing locators migrate with absent root
knowledge; no machine-local root mapping is inferred or persisted. Unresolved
media means a representation has a required resource with no durable locator.
It deliberately does not stat paths or reinterpret an offline observation.

Stale-artifact pages bound the number of candidate outputs examined as well as
the allocation returned by one call. A sparse page can therefore contain fewer
stale results while still returning a continuation. Changed-object queries
deduplicate metadata-capable semantic targets touched after the supplied
revision sequence; media-root lifecycle events identify the production, while
locator lifecycle events identify their owning resource.

A query whose predicate spans tables cannot be proportional to its page through
an ordinary index when matches are sparse: SQLite would walk the stable key and
probe the predicate for every row in the production. Schema 12 therefore adds
derived query-support tables keyed by predicate and stable key — required
memberships without a locator, representations reachable under each recorded
logical root, and activity outputs keyed by activity kind and tool identity.
Triggers maintain them on every write to the authoritative rows, the migration
backfills them, and nothing else writes them, so they cannot diverge through a
forgotten write path. Activities are immutable complete facts, so copying their
kind and tool onto output keys never goes stale. The schema 11 indexes those
tables replace are dropped.

Two queries remain bounded by something other than the page. Stale-artifact
pages evaluate at most one page of candidate outputs, so their cost is the page
size times one bounded artifact evaluation. Changed-object pages deduplicate the
journal suffix after the supplied sequence, so their cost grows with the number
of events after that cursor, not with the production; callers keep that suffix
short by advancing their cursor.

A caller that already holds an identity, such as an adapter translating a
host-object reference, reads the object directly rather than searching a page.
Point reads load one asset, representation, or job by identity and report an
absent identity as not found. Because the compound-media model allows several
representations to share a resource, a resource does not name one owner: the
representations using a resource are a bounded page in representation-identity
order, backed by the existing resource-to-representation membership index. The
C ABI returns a point read as a one-element result set of the same kind a page
returns, so no new result type or accessor family is needed.

Every native surface projects a page the same way. In C a query returns one
owned result-set handle, released once, that exposes the item count and items,
a nullable next-cursor string borrowed from that handle, and, for traversals, a
separate truncation flag. The C++ wrapper, Python, and Rust return an owned page
value with items, an optional cursor, and the truncation flag; the CLI prints
the same three fields as a JSON page object. Whole-set enumerations in the
integration-preview subset remain until that subset is revised.

Pages are weakly consistent across commits. A later page sees current durable
state after its key; callers that need change tracking use the semantic
revision feed. A concurrent insertion before an already-consumed key is not
retroactively included.

## Alternatives considered

Offsets were rejected because their cost grows with page position and their
meaning shifts under concurrent writes. Returning complete vectors with only a
maximum-result error was rejected because callers could not make progress
through a large valid result. Backend-neutral numeric row cursors were rejected
because they leak storage identity. A generic predicate or relationship query
language was rejected in favor of named, typed domain operations.

## Standards impact

OpenAssetIO relationship queries also return pages and opaque page tokens. The
shape is compatible with a later adapter, but PostProject tokens and
relationship values are its own contract and are not OpenAssetIO tokens or
trait data. No media, metadata, provenance, or identifier standard assigns
semantics to local query pagination or to the optional locator/root association.

## Consequences

Large callers can bound allocation and resume queries uniformly on every
surface. Dependency traversal exposes incomplete results instead of hiding a
reached bound. Existing whole-set `dependents()` and `jobs()` calls are replaced
before 1.0; the changelog identifies the paginated replacements. Every new
production-sized query must use this pattern and have an index and scale test
appropriate to its ordering and predicates.
