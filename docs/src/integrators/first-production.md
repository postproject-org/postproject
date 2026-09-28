# Create a production and import media

A production is the durable container for everything PostProject knows about a
project's media. An integration typically creates one production file, opens a
transaction, imports media, and commits. Nothing staged in the transaction is
visible to readers before the commit, and an uncommitted transaction discards
its work.

The example below:

1. creates a production file with an optional display name;
2. begins a transaction;
3. attaches an optional *revision context*, which records which tool made the
   change and why, so the [revision feed](revision-feed.md) can attribute it;
4. imports one original media file, which creates a logical asset with one
   single-file representation and returns the new asset ID;
5. commits the transaction; and
6. lists the asset's representations.

```{code-variants} create-production
```

Choose the language with the tabs or with the **Code** selector in the sidebar;
the choice applies to every example on the site and is remembered in this
browser.

## Reopen a production

A production file outlives the process that created it. Reopening applies any
pending schema migration first, then returns the same production identity. The
example reopens the file, checks that a stored asset still exists, lists every
asset, and reads the newest revision, which is where a [revision
feed](revision-feed.md) consumer starts:

```{code-variants} open-production
```

Whole-set asset listing suits small productions and tests. Hosts that may open
large productions page through assets instead; see [bounded
queries](bounded-queries.md).

## Commit and roll back explicitly

A transaction groups one semantic change. Everything staged in it becomes
durable together on commit, as one revision, or not at all. Rolling back — or
releasing an open transaction — discards the staged work and creates no
revision. Only one transaction may be open per production at a time.

```{code-variants} transaction-lifecycle
:::{no-variant} cli
Every CLI command runs as exactly one transaction: it commits when the command
succeeds and leaves the production unchanged when it fails. There is no
multi-command transaction.
:::
```

Set the revision context before commit. It records the integrating tool and a
short message on the resulting revision; it does not authenticate anyone.

## Handle errors

Every surface reports the same stable error categories — invalid argument, not
found, already exists, I/O, storage, migration, conflict, ambiguous resolution,
fingerprint, unsupported, and internal — in its own idiom. Branch on the
category, never on message text, which is diagnostic and may change:

```{code-variants} error-handling
```

When a staging call fails inside a transaction, roll the transaction back
unless the integration deliberately continues with the rest of the change.
`conflict` also reports lifecycle misuse, such as a second open transaction or
a call on a closed transaction.

## Handles, errors, and lifetimes

The language surfaces differ only in how they express ownership and failure:

- The C ABI returns a status code from every fallible function and reports
  details through an optional error handle. Every handle returned to the
  caller, including result sets and error handles, must be released with its
  documented release function. Strings returned by a result-set accessor borrow
  the result set.
- The C++17 wrapper owns handles with RAII types, copies results into values,
  and returns `postproject::Result<T>`. `POSTPROJECT_TRY` and
  `POSTPROJECT_TRY_ASSIGN` pass a failure on to the caller in one line, and
  `value()` throws `postproject::Exception` when exceptions are enabled.
- The Python binding raises typed exceptions. A transaction used as a context
  manager commits on a clean exit and rolls back when an exception escapes.
- Rust storage returns `postproject_core::Result`. The media adapter prepares
  an import from the filesystem before the transaction stages it.
- The CLI commits each command as one transaction. Pass `--json` for
  structured output that scripts can parse.

Every example on this site comes from a program that CI compiles and runs
against an installed package, so the listings stay in step with the public
interfaces. See the [C quickstart](c-quickstart.md),
[C++ quickstart](cpp-quickstart.md), and [Python quickstart](python.md) for
building and running a consumer, and [install a release](installing-a-release.md)
for the packages.
