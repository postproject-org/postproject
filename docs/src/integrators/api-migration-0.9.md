# Move an integration from 0.8 to 0.9

The 0.9 development SDK uses C ABI 57, schema 27 and exchange protocol 1.
Update the binding and native library together. Python 0.8 bindings reject
an ABI 57 library. Existing production identities and stored knowledge retain
their meaning; opening older files follows the existing migration rules.

| Old form | Use in 0.9 |
|---|---|
| C additive/revision-only transaction constructors | `pp_production_begin_edit(production, base_or_NULL, context_or_NULL, ...)` |
| C++ `beginTransaction()` and context setters | `production.edit(context)` / `production.edit(base, context)` / `view.edit(context)` |
| Python edit context setter | Pass `origin=` and `message=` to `edit()` |
| Python `production.transaction()` / `Transaction` | `production.edit()` / `Edit` |
| Python revision-ID-only transaction base | `production.edit(view.decision_base)` |
| Python `FileSource(path)` | The path itself |
| Asset-only Python/C++ import result | `ImportedMedia.asset_id` and `.representation_id` |
| C import's asset-only outputs | Add nullable `out_representation_id` before the error output |
| Python `find_known_media_by_locator` / `find_known_media_by_fingerprint` | `find_known_media(path_or_evidence, limit=...)` |
| C++ `findKnownMediaByLocator` / `findKnownMediaByFingerprint` | `findKnownMedia(path_or_evidence, limit)` |
| Python `production.assets` | `view.assets(page_size=100)` inside a read session |
| Python `id in production.assets` | `production.asset_exists(id)` |
| Manual asset/revision page loops in Python or Rust | The retained view's `assets` / `revisions` scans |
| C file-source creation, staging and release | `pp_transaction_import_file` / `pp_transaction_add_file_representation` |
| C resolution factory plus setters | One `pp_resolution_config_t` passed to creation |
| C++ resolution factory plus setters | `ResolutionOptions::create(ResolutionConfig)` |
| C++ `MediaSource::file(path)` | `std::filesystem::path` or UTF-8 path string |
| Rust `commit_with_receipt()` / C++ `commitWithReceipt()` | `commit()` |
| C status-only commit / `pp_transaction_commit_with_receipt` | `pp_transaction_commit(edit, &receipt, &error)` |
| CLI `--base-revision ID` | `--decision-base TOKEN` from `inspect` |

Python metadata accepts `(vocabulary, property)` keys and native `str`,
`bool`, signed `int`, `bytes` and exact `Fraction` values. Reads still return
lossless typed values. Keep explicit unsigned, URI, localized text and other
ambiguous variants. Floats, `None`, plain lists and dictionaries do not acquire
an inferred metadata meaning. See {doc}`metadata-vocabularies`.

Python `edit()`, `edit(base)` and `view.edit()` accept `origin=` and
`message=`. Uncommitted context exit rolls back. Commit returns this edit's
own receipt, including an absent revision for no change. A failed commit is
terminal; {doc}`semantic-conflicts` explains explicit retry.

## Five Python tasks

These excerpts run together in `docs/examples/python/ergonomics.py`. Its
fixture supplies `rushes/A001.mov`, an empty `moved/` directory and an output
directory. The host produces the output bytes before recording them.
The {download}`historical comparison <../../evidence/release-0.9/python-before.py>`
runs the same workflow against the published 0.8 platform wheel.

Import a file and ordinary metadata with one edit:

```{literalinclude} ../../examples/python/ergonomics.py
:language: python
:start-after: "# [ergonomic-import]"
:end-before: "# [/ergonomic-import]"
:dedent:
```

Reuse an existing identity after an explicit adoption decision:

```{literalinclude} ../../examples/python/ergonomics.py
:language: python
:start-after: "# [ergonomic-adopt]"
:end-before: "# [/ergonomic-adopt]"
:dedent:
```

After the fixture moves the file, resolve and explicitly confirm its location.
The detached base lets the caller close the view before host/UI work:

```{literalinclude} ../../examples/python/ergonomics.py
:language: python
:start-after: "# [ergonomic-resolve]"
:end-before: "# [/ergonomic-resolve]"
:dedent:
```

Record a produced representation with its actual activity and parameters:

```{literalinclude} ../../examples/python/ergonomics.py
:language: python
:start-after: "# [ergonomic-derived]"
:end-before: "# [/ergonomic-derived]"
:dedent:
```

Activity inputs describe provenance. They do not imply authored references
inside the output; record [dependency observations](dependencies.md) only when
the host has inspected such references.

Scan a pinned asset set and finite history with bounded pages:

```{literalinclude} ../../examples/python/ergonomics.py
:language: python
:start-after: "# [ergonomic-revisions]"
:end-before: "# [/ergonomic-revisions]"
:dedent:
```

For the equivalent C, C++17, Rust and CLI paths, use {doc}`first-production`,
{doc}`known-media-adoption`, {doc}`media-resolution`, {doc}`provenance` and
{doc}`revision-feed`. The CLI JSON format remains 1; command-line decisions
now use the scoped token. See {doc}`../reference/cli-output`.

### Produced outputs (development ABI 56)

Use `Edit.register_outputs`, C++ `Transaction::registerOutputs`, native
`pp_transaction_register_outputs` or CLI `artifact register` for an
already-produced file or compound source and its actual activity/parameters.
Rust uses `prepare_outputs` followed by owned `stage`; failed staging rolls
back the owned edit. Commit remains explicit.

`ActivitySpec` writes accept representation IDs or `(ID, role)` pairs. Copied
read edges retain snapshots and no longer serve as write builders. Python
parameters accept a mapping or repeated key/value pairs; C++ accepts typed
parameter tuples. Native activity creation now takes `pp_activity_recipe_t`
and a separate output-edge array. Update headers, library and bindings together.

### Verified relinking (development ABI 57)

Retain the import's representation ID and select it explicitly from resolution
results. Python `select_verified`, C++ `selectVerified` and native
`pp_resolution_set_select_verified` copy/lend complete verified locations with
the original decision base. Confirm them together using a read-bound edit;
Rust uses `stage_relink`. CLI `media relink REPRESENTATION --confirm` captures
its own coherent decision and requires explicit confirmation.

A single candidate is insufficient when verification is partial or search is
incomplete. Unverified optional members remain in diagnostics. Existing
locations are guarded no-ops. Explicit locator confirmation remains available
for independent human/host decisions. Deferred intent uses
`locator.confirm.v1`; older protocol decoders reject that command as unsupported.

### Native Python metadata values

Read and write strings, signed integers, booleans, binary values and exact
rationals as `str`, `int`, `bool`, `bytes` and `Fraction`. Lists and tuples write
one list value; reads return immutable tuples. The corresponding
`MetadataString`, `MetadataI64`, `MetadataBool`, `MetadataBytes`,
`MetadataRational` and `MetadataList` classes are removed.

Unsigned values keep `MetadataU64`; URI, language, timestamp, decimal, reference
and ordered structure values remain explicit. Inspect Python types when reading:
ordinary Python equality equates `True` and `1`, while metadata assertion/field
equality preserves their distinct stored kinds. Repeated assertions and ordered
fields retain their meaning. Cycles and unsupported dictionaries are rejected.

### Advanced Python imports

Import job requests, states, leases and job events from `postproject.jobs`.
Import proposal commands/builders, request IDs, durable outcomes and replication
positions from `postproject.exchange`. Their package-root exports are removed;
ordinary production/media/metadata recipes keep the root imports.

`Edit.prepare_proposal` captures an already prepared native edit. The command
builder expresses deferred intent. Both remain available. Notification progress
and replication positions retain their distinct scopes.
