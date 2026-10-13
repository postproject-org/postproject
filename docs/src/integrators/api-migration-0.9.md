# Move an integration from 0.8 to 0.9

The 0.9 development SDK uses C ABI 54, schema 27 and exchange protocol 1.
Update the binding and native library together. Python 0.8 bindings reject
an ABI 54 library. Existing production identities and stored knowledge retain
their meaning; opening older files follows the existing migration rules.

| Old form | Use in 0.9 |
|---|---|
| Python `production.transaction()` / `Transaction` | `production.edit()` / `Edit` |
| Python revision-ID-only transaction base | `production.edit(view.decision_base)` |
| Python `FileSource(path)` | The path itself |
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
