# 0083: Use ordinary Python values for exact metadata reads and writes

Status: accepted for development Python 0.9.

Metadata writes already accept ordinary scalar values, but reads wrap the same
values in trivial classes. Replace string, signed-integer, boolean, binary,
rational and list wrappers with `str`, `int`, `bool`, `bytes`, `Fraction` and
tuples on reads. Writes accept bounded sequences for one list-valued assertion;
binary buffers are copied as bytes.
Remove the superseded classes and migrate current callers; no aliases remain.

Plain integers always mean signed 64-bit values. Keep `MetadataU64` for unsigned
values, and explicit URI, language string, timestamp, decimal, reference and
ordered structure forms. A list value remains one assertion; repeated fields
remain ordered structure fields rather than a dictionary. Native values retain
their Python types, including boolean versus integer and rational versus integer.
Assertion/field equality and hashing preserve those distinctions recursively.

Reject excessive nesting, oversized collections and cyclic input containers
before recursively allocating native children. Reading creates no new native
owners. Existing queries, stored tags, exact values and captured/replayed effects
keep their meaning. Decimal and timestamp conversion stays explicit.

Schema 27, C ABI 57 and exchange framing 1 are unchanged. Standards-policy
review: this is a language projection, with no vocabulary, cardinality,
identifier, normalization or wire-encoding change.
