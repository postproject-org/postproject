# Bounded queries and cursors

Production-sized queries return stable keyset pages instead of complete lists.
Choose a page size from 1 through 1,000 and pass the returned opaque cursor to
the next call. Keep the query root, filters, page size, and traversal bounds
unchanged while following a cursor.

A cursor is a weakly consistent continuation, not a durable snapshot or a
revision-feed position. A later page sees the current durable state after its
key. Use the [revision feed](revision-feed.md) when an integration needs to
observe changes rather than scan current state.

## Available queries

Every named query below is available on all five surfaces as of C ABI version 25
and package `0.4.0-alpha.1`. The CLI prints a page as items followed by a
`next_cursor` line, or as a JSON object with `items`, `next_cursor`, and
`traversal_truncated`; `--limit` defaults to 100 and `--cursor` continues a
page.

| Query | C | C++ | Python | Rust | CLI |
|---|---|---|---|---|---|
| Assets | `pp_production_assets_page` | `assets(limit)` | `assets_page` | `assets_page` | `media list --limit` |
| Representations of an asset | `pp_production_representations_page` | `representations(id, limit)` | `representations_page` | `representations_page` | `representation list` |
| Resources of a representation | `pp_production_resources_page` | `resources` | `resources_page` | `resources_page` | `representation resources` |
| Locators of a resource | `pp_production_locators_page` | `locators` | `locators_page` | `locators_page` | `locator list` |
| Unresolved media | `pp_production_unresolved_media` | `unresolvedMedia` | `unresolved_media` | `unresolved_media` | `media unresolved` |
| Representations under a media root | `pp_production_representations_under_media_root` | `representationsUnderMediaRoot` | `representations_under_media_root` | `representations_under_media_root` | `media under-root` |
| Objects carrying a metadata property | `pp_production_query_metadata` | `queryMetadata` | `query_metadata` | `metadata_query` | `metadata find` |
| Activities producing or consuming | `pp_production_activities_producing_page`, `pp_production_activities_consuming_page` | `activitiesProducing(id, limit)`, `activitiesConsuming(id, limit)` | `activities_producing_page`, `activities_consuming_page` | `activities_producing_page`, `activities_consuming_page` | `activity producing`, `activity consuming` |
| Outputs by activity kind or tool | `pp_production_outputs_by_activity_kind`, `pp_production_outputs_by_tool` | `outputsByActivityKind`, `outputsByTool` | `outputs_by_activity_kind`, `outputs_by_tool` | `activity_outputs` | `activity outputs --kind` or `--tool-name` |
| Provenance ancestors or descendants | `pp_production_provenance_ancestors_page`, `pp_production_provenance_descendants_page` | `ancestors(id, depth, …)`, `descendants(id, depth, …)` | `provenance_ancestors_page`, `provenance_descendants_page` | `ancestors_page`, `descendants_page` | `activity ancestors`, `activity descendants` |
| Dependencies or dependents | `pp_production_dependencies`, `pp_production_dependents` | `dependencies`, `dependents` | `dependencies`, `dependents` | `dependencies`, `dependents` | `dependency dependencies`, `dependency dependents` |
| Stale artifacts | `pp_production_stale_artifacts` | `staleArtifacts` | `stale_artifacts` | `stale_artifacts` | `artifact stale` |
| Jobs by state and kind | `pp_production_jobs` | `jobs` | `jobs` | `jobs` | `job list` |
| Objects changed since a revision | `pp_production_objects_changed_since` | `objectsChangedSince` | `objects_changed_since` | `objects_changed_since` | `revisions changed` |

The complete-list reads for assets, representations, metadata properties,
producing and consuming activities, and provenance ancestors and descendants
remain available on C, C++, Python, and Rust for small productions. Their cost
grows with the production, so prefer the paginated query in new code. The CLI
`media list` returns the complete asset list unless `--limit` or `--cursor` is
given; its other query commands always return one page.

## Read one object by identity

An integration that already holds an identity, for example from a
[host-object binding](host-object-bindings.md) or an OpenAssetIO entity
reference, reads that object directly instead of searching a page. Reading an
asset, a representation, or a job returns the one object or reports *not
found*. A resource is not owned by exactly one representation — the compound
model allows several representations to share it — so the representations
using a resource are a bounded page like any other query. These reads are
available on all five surfaces as of C ABI version 27; the C ABI returns a point
read as a one-element result set of the usual kind.

```{code-variants} point-reads
```

| Read | C | C++ | Python | Rust | CLI |
|---|---|---|---|---|---|
| One asset | `pp_production_asset` | `asset` | `asset` | `asset` | `media show` |
| One representation | `pp_production_representation` | `representation` | `representation` | `representation` | `representation show` |
| One job | `pp_production_job` | `job` | `job` | `job` | `job show` |
| Representations using a resource | `pp_production_representations_using_resource` | `representationsUsingResource` | `representations_using_resource` | `representations_using_resource` | `representation using-resource` |

## Page media structure

Asset pages follow creation time and asset ID. Representation pages list one
asset's representations, and resource pages list one representation's resources
in structural order. Locator pages list one resource's locators together with
the owning resource and the logical media root recorded when the locator was
confirmed. This example drains the asset pages and, for brevity, reads the first
page of each nested query:

```{code-variants} media-structure-pages
```

A locator records a root only when it was confirmed under that root. Pass the
candidate's media root to `pp_transaction_confirm_locator`, C++
`confirmLocator`, Python `confirm_locator`, or Rust `prepare_confirmed_locator`,
as the
[confirmation example](media-resolution.md) does. The CLI
`media resolve --confirm` records that root automatically. Imported locators,
locators confirmed without a root, and locators stored before schema version 11
report no root; PostProject never infers one from a machine-local path.

C, C++, and Python resource pages return resource IDs; Rust returns complete
resource values, and the CLI prints each resource's ID, size, and fingerprints.

## Query recorded media knowledge

Two media queries answer questions from what the production has recorded:

- *unresolved media* lists representations with a required resource for which
  the production records no locator at all; and
- *representations under a media root* lists representations with at least one
  locator recorded under the named logical root. The root must be configured in
  the production.

```{code-variants} knowledge-only-media
```

Both queries read recorded knowledge only. They never stat, open, or search the
filesystem, and they do not map a root name to a local directory. A
representation whose recorded locator points at a moved or deleted file is not
unresolved, and a representation under a root may be offline on this machine.
Current filesystem state stays with [resolution](media-resolution.md) and the
non-mutating inventory scan (`postproject media inventory`).

## Query metadata by property

A metadata query finds assertions of one exact vocabulary and property. An
optional exact value restricts the page to one scalar value: text, a
language-tagged string, an integer, a decimal, a rational, a boolean, a
timestamp, a URI, bytes, or an object reference. The comparison is exact on the
typed value, without case folding, numeric normalization, or comparison across
value kinds, so a language-tagged string matches only the same text and
language tag. A list or structure predicate is an invalid argument.

```{code-variants} metadata-query-pages
```

To match every value, C passes `NULL`, C++ omits the value argument, Python
omits `value`, and Rust passes `None`; the CLI omits `--value-file`. The CLI
reads an exact value from the tagged JSON file passed with `--value-file`, using
the shape shown in
[metadata vocabularies](metadata-vocabularies.md).

## Query provenance

Producing and consuming activity pages return activities connected to one
representation. Output queries return the distinct representations produced by
activities of one exact kind, or by activities with one exact tool identity.
Tool matching compares name, version, and URI; an absent version or URI matches
only activities that recorded none.

Ancestor and descendant queries follow activity inputs or outputs. Like
dependency traversal, they require a maximum depth from 1 through 64 and a
visited-representation bound from 1 through 1,000, including the starting
representation. Each match reports its shortest observed depth, and pages are
ordered by representation ID.

```{code-variants} provenance-query-pages
```

The CLI traversal commands default to `--max-depth 64` and
`--max-representations 1000`.

## Find stale artifacts

A stale-artifact query evaluates activity outputs as
[artifact evaluation](../concepts/artifact-knowledge.md) does, with the supplied
depth and representation bounds applied to each evaluation, and returns those
currently evaluated as stale. An optional source representation restricts the
candidates to its provenance descendants.

```{code-variants} stale-artifact-pages
```

A stale-artifact page bounds the number of candidate outputs examined, not only
the number returned. A page can therefore contain fewer stale results than its
page size, or none, and still carry a continuation; keep following the cursor
until none is returned. With a source, `traversal_truncated` reports that the
descendant traversal reached its bound and candidates may be missing. The CLI
`artifact stale` command defaults to `--max-depth 64` and
`--max-representations 1000`.

## List objects changed since a revision

The changed-object query returns each distinct production, asset,
representation, resource, activity, or job touched by a revision after the
supplied sequence, derived from the semantic event journal. Media-root events
report the production, and locator events report the owning resource.

```{code-variants} changed-objects
```

Use it to refresh cached objects after reconnecting. It answers what changed,
not how; read the [revision feed](revision-feed.md) when event order and meaning
matter.

## Traverse dependencies

Forward queries start at a representation. Reverse queries start at an asset
or representation and return the representations that depend on it. Both
directions require a maximum depth and a visited-representation bound. Each
match includes its shortest observed depth.

```{code-variants} dependency-queries
```

`traversal_truncated` and `next_cursor` answer different questions. A truncated
traversal stopped at a graph bound and may omit matches. A next cursor means
another output page exists within the graph that was actually traversed.

## Filter and page jobs

Job queries accept optional exact state and open-world kind filters. This
example requests two jobs, then drains one-item pages using the same filters on
every call:

```{code-variants} job-query-pages
```

## Cursor ownership

The C cursor borrows its result-set handle and must be copied before releasing
that handle. C++, Python, and Rust return owned cursor values. The CLI emits the
cursor as `next_cursor` in its JSON page object.

Passing a cursor to another named query, root, filter set, page size, or
traversal bound is an invalid argument. Treat cursor contents as private and do
not parse or synthesize them.
