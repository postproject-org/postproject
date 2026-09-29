# Metadata assertions

PostProject stores metadata as assertions about an object. Each assertion has
three distinct parts:

```text
vocabulary + property + typed value
```

The vocabulary identifies who defines the term. The property identifies the
term inside that vocabulary. The value carries the data without collapsing its
type into plain text. This design lets PostProject retain standardized and
application-specific metadata without inventing a broad media vocabulary of its
own.

An assertion can target a production, asset, representation, resource, activity,
or job.
Metadata is not copied automatically between these identity levels.

## Repetition and structure

A property may have several independently ordered assertions. For example, two
keywords are two values of the same property. A list is instead one value whose
contents are ordered. PostProject preserves that distinction.

Values can be:

- plain or language-tagged text;
- signed and unsigned integers, exact decimal or rational numbers, and booleans;
- timestamps and URIs;
- opaque bytes;
- ordered lists and ordered named structures;
- references to PostProject objects.

Floating-point numbers and unconstrained JSON are not canonical persisted
forms. The SQLite backend uses a private, deterministic, versioned binary
encoding.

## Unknown terms

Core does not require a vocabulary package to retain a property. Unknown
vocabulary and property identifiers round-trip exactly. A future registry may
add labels, cardinality hints, expected types, or validation, but absence of
that registry must not discard data.

## Safety limits

Values are validated before persistence. Current limits include 32 nested
levels, 4,096 direct list items or structure fields, one mebibyte per text
value, 15 mebibytes per binary or aggregate payload, and a bounded decimal
scale. Stored encodings are decoded as untrusted input; invalid tags, lengths,
UTF-8, URIs, language tags, object kinds, truncation, and trailing bytes return
errors rather than panicking.

## Across public surfaces

This example writes a language-tagged title and reads it through both object
and property-oriented views:

```{code-variants} metadata
```

Every value kind round-trips with its type, including nested lists, structures,
and object references:

```{code-variants} typed-metadata
```
