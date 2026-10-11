# 0071: Python metadata accepts unambiguous native inputs

Status: accepted for development.

## Decision

All metadata writes, proposal commands and exact scalar queries share one
checked conversion. A `(vocabulary, property)` string tuple supplies the key.
Strings mean plain text, booleans mean booleans, integers mean signed 64-bit
values, bytes remain bytes, and `Fraction` means an exact rational with signed
64-bit numerator and unsigned 64-bit positive denominator. Check booleans
before integers. Preserve exact strings without namespace normalization.

Explicit tagged values still express unsigned integers, decimal coefficients
and scales, timestamp units, URIs, language tags and reference kinds. Ordered
`MetadataList` and `MetadataStruct` accept mixed native/tagged children and
preserve repeated field names. Reject floats, `None`, arbitrary iterables and
plain dictionaries; do not infer their meaning or silently round them.

Read results retain tagged `MetadataValue` objects and `MetadataProperty`
keys. These distinguish exact stored kinds and can be reused as inputs; they
are the lossless domain representation, rather than compatibility aliases.
The native input path uses the same validators and effect capture as tagged
inputs, including proposal preparation, retries and mirror replay.

## Migration and standards impact

Use `(vocabulary, name)` and native scalars in common writes. Keep explicit
tags for ambiguous kinds and when inspecting reads. The input aliases
`MetadataKey` and `MetadataInput` describe accepted Python values and have no
runtime ownership or storage model.

Reviewed against the contributor standards policy: no metadata encoding,
schema, C ABI or exchange version changes. Vocabulary IDs, ordered values
and all existing kinds retain their meaning; no standards mapping is inferred.
