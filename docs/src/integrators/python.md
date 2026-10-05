# Python quickstart

The Python package uses the installed public C ABI through the standard
library's `ctypes` module. It requires Python 3.11 or newer and does not build
or import Rust code.

Install the platform wheel for your system, and the binding loads the native
library inside it (see [Install a release](installing-a-release.md)). With the
platform-neutral wheel, point the binding at an exact native library instead:

```sh
export POSTPROJECT_LIBRARY=/opt/postproject/lib/libpostproject.so
python /opt/postproject/share/doc/postproject/examples/python/quickstart.py \
  production.pproj \
  /opt/postproject/share/doc/postproject/examples/fixtures/sample-media.dat
```

Applications may instead pass `library_path=` to `Production.create` or
`Production.open`; the quickstart does so when given `--library PATH`. An
explicit path wins, then `POSTPROJECT_LIBRARY`, then a platform wheel's own
library. The binding does not search the working directory or modify the
platform loader path, and it loads each library once per process. The
installed quickstart above runs in package CI on Linux, macOS, and Windows.

A transaction context requires explicit `commit()`. Every uncommitted exit
rolls back, including normal exit. `close()` is idempotent for production and
transaction handles, and a
finalizer is a fallback for handles that were not closed explicitly.
The {doc}`coherent-reads` recipe uses `Edit` contexts with the same lifecycle.

Identity annotations such as `AssetId` are `NewType` hints over ordinary
`uuid.UUID` values. Use `parse_id(text, AssetId)` to parse text, standard UUID
operations at serialization boundaries, and optional type checking to catch
wrong-kind arguments. Use explicit `AssetRef(asset_id)` or
`RepresentationRef(representation_id)` variants for metadata, dependency,
external-identifier, and host-binding targets. A runtime UUID does not retain
its nominal type; the store checks existence and production membership.

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
locators with their sequence namings, and distinct resource and representation fingerprints as immutable
values. `production.media_roots` lists immutable logical-root summaries in
resolver order. Root creation takes a portable name rather than a directory;
local paths are supplied to `resolve()`. Root creation, enablement, removal,
locator retirement, and explicit
candidate confirmation are transactional through `add_media_root()`,
`set_media_root_enabled()`, `remove_media_root()`, `retire_locator()`,
and `confirm_locator()`, which also records the logical root a candidate was
found under and, for an image sequence, the `SequenceNaming` of its files. A media source — `FileSource`,
`ImageSequenceSource`, `OrderedPartsSource`, or `PackageSource` — describes a
representation's content structure: `import_media()` creates an asset whose
original has that structure, and `add_representation()` adds a representation
of a chosen kind to an existing asset. Both also accept a plain path as a
single file. The generated low-level declaration table covers every
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

## Plug-ins of a host application

Applications that embed Python, such as Blender, Nuke, Houdini, or Maya, often
share one Python environment among all their plug-ins. Ship PostProject with a
plug-in this way:

- **Bundle the platform wheel** and pass no library path. The wheel's binding
  and library always match.
- **Expect to share it.** Blender, for example, installs the wheels of all
  extensions into one `site-packages` and keeps only the newest wheel of each
  name. Another plug-in may bring a newer PostProject than yours, and before
  1.0 its Python API may differ from the one you built against.
- **Never set `POSTPROJECT_LIBRARY`** from a plug-in. The variable is
  process-wide and would override every other plug-in's library.

The [Blender pilot](https://github.com/postproject-org/postproject-blender)
is an extension packaged this way.
