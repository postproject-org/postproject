# Metadata vocabularies

Metadata properties are identified by an exact vocabulary string and an exact
vocabulary-local property string. Do not rewrite either string during import.
Use a standard namespace when a standard already defines the concept; use a
stable application-owned namespace only for genuinely application-specific
data.

## Add and read metadata

A metadata assertion attaches a typed value to a production, asset,
representation, resource, activity, or job under an exact vocabulary and
property. Every surface can:

- append a value, so a property can hold several ordered values;
- remove every value of a property from a target (except the C++ wrapper; see
  below);
- read every assertion on a target (except the C++ wrapper; see
  [availability](#availability)); and
- find every assertion that uses an exact vocabulary and property, as one
  [bounded page](bounded-queries.md) at a time and optionally restricted to an
  exact scalar value.

The Rust storage API can additionally replace every ordered value of a property
atomically. The example adds a language-tagged title to an asset and reads it
back from both directions:

```{code-variants} metadata
```

All writes belong to an explicit production transaction. A failed operation or
rollback leaves no partial assertions.

## Typed values

Values are typed rather than stringly encoded: plain and language-tagged text,
signed and unsigned 64-bit integers, exact decimals and rationals, booleans,
timestamps, URIs, opaque bytes, typed object references, and recursively nested
ordered lists and named-field structures. Every surface preserves every value
kind on read.

The example writes one value of every kind under an application-owned
vocabulary, reads them back by dispatching on the value type, and pages through
a property query:

```{code-variants} typed-metadata
```

In C, recursive input handles copy their children, so callers can release
intermediate list and structure values immediately after construction. In C++,
`MetadataValue` is one copyable type for writing and reading, like Python's
value classes: read one with `getIf<T>()` or by visiting `variant()`, and
compare values with `==` (ADR 0036). The CLI
reads a typed value from a JSON file in the same tagged shape that `--json`
output emits. Decimal coefficients are strings so JSON consumers do not lose
precision, binary values use hexadecimal text, and lists and structure fields
are recursive and ordered.

## Remove a property

Removal deletes every value of one property from one target in a read-bound
edit. Unbased transactions may append values; replacement and removal require
a decision base. A destructive commit conflicts after any intervening change
to that property, including an append. Reread before retrying.

```{code-variants} remove-metadata
```

## Query by property and value

`find` and the paged property query return every assertion that uses an exact
vocabulary and property. An optional exact scalar value narrows the page; see
[bounded queries](bounded-queries.md#query-metadata-by-property) for
the paging contract.

## Technical inspection

The optional `ffprobe` adapter stores its results as ordinary typed assertions
under `https://postproject.org/ns/technical-media/1` with property
`inspection`; see [fingerprints, verification, and
inventory](fingerprints-and-verification.md#record-technical-inspection). No
FFmpeg type or dependency enters `postproject-core`, and raw embedded tag keys
and values are kept as ordered key/value structures so unfamiliar tags do not
need to become schema fields. Every surface reads the resulting assertion
through its normal metadata reads.

`media resolve --verify` reuses a single stored inspection as partial identity
evidence when scoring relocated file candidates. The candidate remains
ambiguous if another credible match exists. Subprocess output is limited to
8 MiB per stream, execution defaults to a 30-second deadline, JSON and numeric
values are parsed without floating point, and stderr diagnostics are truncated.
Missing `ffprobe`, non-zero exit, timeout, oversized output, and malformed JSON
are distinguishable outcomes.

## Optional Rust registry

The core registry supplies a small set of advisory definitions for IPTC Video
Metadata Hub JSON, Dublin Core, XMP Basic, EBUCore, and PostProject-owned
metadata. A property hint can describe accepted value kinds, cardinality,
labels, descriptions, and known mapping aliases. `validate_values` applies
those rules only when an application explicitly calls it.

Registry lookup uses exact identifiers. An absent vocabulary or property is
not an error, and persistence never invokes the registry automatically. This
keeps unknown and application-specific metadata fully round-trippable.

## Availability

The typed domain model, optional vocabulary registry, and SQLite persistence
back every surface. C, Python, Rust, and the CLI read and write every value
kind. The C++ wrapper writes every value kind and reads property queries with
`queryMetadata`, but does not wrap reading every assertion on one target or
removing a property, and does not expose its native handles; C++ integrations
call `pp_production_metadata` and `pp_transaction_remove_metadata_property`
through the C API for that. Activity metadata
is writable after the activity is created in the same or an earlier
transaction.

See [standards boundaries](../concepts/standards-boundaries.md) and the
[mapping matrix](../reference/standards-mapping-matrix.md) for the intended
relationship to IPTC Video Metadata Hub, EBUCore, XMP, and other standards.
