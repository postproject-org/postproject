# ADR 0036: Readable C++ metadata values

- Status: Accepted
- Date: 2026-09-28

## Context

The C++ wrapper represented a metadata value as `MetadataInput`, a move-only
handle to a native input. Reads converted every stored value back into such a
handle. That covered metadata queries, activity and job metadata, and the
parameters of a regeneration plan. The handle could be passed to an exact-value
query or copied to another target, but its value could not be read. The C ABI
has a typed getter for every value kind, and Python returns frozen value
classes, so only C++ hosts were cut off from their own data.

The Kdenlive pilot recorded its render arguments as an ordered list on each
proxy activity, and could not read them back. The gap is general. A C++ host
cannot use its own data in several situations:

- A worker handed a regeneration plan cannot read the parameters it must
  render with.
- An OpenAssetIO manager or asset browser cannot show metadata.
- An editor cannot read back an identifier or setting it stored on an asset.
- A test cannot compare a value it wrote with the value it reads.

## Decision

`MetadataValue` replaces `MetadataInput` as the one C++ type for writing and
reading metadata, as Python's value classes already are.

- It is a copyable value holding a `std::variant` of alternatives:
  - `MetadataString` and `MetadataLanguageString`;
  - `MetadataSignedInteger` and `MetadataUnsignedInteger`;
  - `MetadataDecimal`, with a base-ten coefficient string and a scale;
  - `MetadataBoolean`, `MetadataTimestamp`, `MetadataUri`, and `MetadataBytes`;
  - `MetadataRational`, `MetadataList`, `MetadataStructure` of
    `MetadataField`s, and `MetadataReference`.
- The factories keep their names: `plainString`, `languageString`,
  `signedInteger`, `unsignedInteger`, `decimal`, `boolean`, `timestamp`,
  `uri`, `bytes`, `rational`, `list`, `structure`, and `reference`.
- `getIf<T>()` returns the alternative or null, and `variant()` supports
  `std::visit`. Values and alternatives compare with `==`.
- `MetadataAssertion::value` and `RegenerationParameter::value` are
  `MetadataValue`s, and `addMetadataValue` and `queryMetadata` take one.
- A value is converted to a native input, and validated, when an operation
  consumes it. Invalid text or an invalid decimal is reported by that operation,
  as before. The value itself no longer carries an error.

## Alternatives considered

- **Add read accessors to `MetadataInput`.** The handle would still be
  move-only, and every read would call the C ABI again. A plain value is
  simpler to copy, compare, and keep.
- **Keep a separate read type next to `MetadataInput`.** Two types for one
  concept would force a conversion whenever a read value is written again, for
  example when a regeneration plan's parameters are copied to a new job.
- **Expose `std::variant` directly as the value type.** A recursive variant
  needs a wrapper for its list and structure members anyway. A class also keeps
  the factories and room for later additions.

## Standards impact

None. The value kinds are unchanged (ADR 0004).

## Consequences

C++ hosts read every metadata value on every read path. Code that named
`MetadataInput` or `MetadataFieldInput`, or called `MetadataInput::error()`,
must be updated. The C ABI is unchanged.
