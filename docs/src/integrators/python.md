# Python quickstart

The Python package uses the installed public C ABI through the standard
library's `ctypes` module. It requires Python 3.11 or newer and does not build
or import Rust code.

Point the binding at an exact native library:

```sh
export POSTPROJECT_LIBRARY=/opt/postproject/lib/libpostproject.so
python /opt/postproject/share/doc/postproject/examples/python/quickstart.py \
  production.pproj \
  /opt/postproject/share/doc/postproject/examples/fixtures/sample-media.dat
```

Applications may instead pass `library_path=` to `Production.create` or
`Production.open`; the quickstart does so when given `--library PATH`. The
binding resolves that explicit path and does not search the working directory
or modify the platform loader path. The installed quickstart above runs in
package CI on Linux, macOS, and Windows.

A transaction context commits only after a clean exit. An exception rolls it
back. `close()` is idempotent for production and transaction handles, and a
finalizer is a fallback for handles that were not closed explicitly.

Production operations may run concurrently from multiple Python threads; calls
on one native handle serialize internally. Do not call `close()` concurrently
with an operation, and do not share a transaction between concurrent callers.
Open the production again when reads should use a separate native handle during
a commit.

The quickstart program shows each of these rules:

```{literalinclude} ../../../examples/python/quickstart.py
:language: python
```

The current high-level surface covers production lifecycle, transactions,
original-media import, revision context, iterable asset summaries with identity
membership checks, and the paginated revision feed with typed semantic events.
External identifiers can be added,
removed, enumerated, and found by exact scheme and value. Metadata reads and
writes preserve every typed value kind; scalar, repeated, structured, and
reference values use `transaction.add_metadata()`. Provenance activities can be
created and queried through immutable value objects and keyed graph views.
`production.resolve(asset_id, {"rushes": "/mnt/show/rushes"})` returns typed
representation availability, resource candidates, evidence, diagnostics, and
missing-frame details while supplying machine-local paths for named production
roots. `production.resolutions[asset_id]` is the shorthand when no mappings are
needed.
`production.representations[asset_id]` returns the
stored structure, ordered membership, compact sequence descriptor, resources,
locators, and distinct resource and representation fingerprints as immutable
values. `production.media_roots` lists immutable logical-root summaries in
resolver order. Root creation takes a portable name rather than a directory;
local paths are supplied to `resolve()`. Root creation, enablement, removal,
locator retirement, and explicit
candidate confirmation are transactional through `add_media_root()`,
`set_media_root_enabled()`, `remove_media_root()`, `retire_locator()`,
`confirm_locator()`, and `confirm_locator_under_root()`, which also records the
logical root a candidate was found under. Transactions can add
single-file, compact image-sequence, ordered-parts, and package representations
to an existing asset. The generated low-level declaration table covers every
function and struct in the current ABI.

Production-sized reads are [bounded queries](bounded-queries.md) that take a
keyword `limit` and an optional `cursor` and return a `QueryPage` with `items`,
`next_cursor`, and `traversal_truncated`: `assets_page()`,
`representations_page()`, `resources_page()`, `locators_page()`,
`unresolved_media()`, `representations_under_media_root()`, `query_metadata()`,
`activities_producing_page()`, `activities_consuming_page()`,
`outputs_by_activity_kind()`, `outputs_by_tool()`,
`provenance_ancestors_page()`, `provenance_descendants_page()`,
`dependencies()`, `dependents()`, `stale_artifacts()`, `jobs()`, and
`objects_changed_since()`.
