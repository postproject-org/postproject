# 0061: Representation content values

Status: accepted for the 0.7 development SDK.

## Decision

A representation carries one content alternative: a single resource, an image
sequence with its descriptor, ordered parts, or a package. Rust retains its
checked content value. C exposes the stored kind and checked sequence accessors;
C++ uses a variant and Python a union of owned, frozen content records.

Compound construction copies its members and rejects empty or oversized lists,
duplicate resources, missing roles, optional ordered parts, and packages with no
required member. Sequence construction checks the stepped frame domain, positive
rate and integer ranges. Python exposes the exact rate as `fractions.Fraction`.
Entity existence and production membership remain operation checks.

Inspection properties derive the kind, members and sequence descriptor from the
content. They cannot disagree with a separately stored discriminator. Unknown
required native kinds fail decoding rather than constructing a partial value.

## Migration and standards impact

Python callers constructing `Representation` pass `content` instead of separate
`structure_kind`, `members` and `image_sequence` arguments. Existing read
properties remain available. C++ callers inspect the content variant or derived
accessors instead of those fields.

This changes binding source contracts, not persisted identities, schema, C
layouts or external standard mappings. Unknown namespaced roles retain their
spelling; their existing syntax and membership rules remain authoritative.
