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
rate and integer ranges. Python exposes the exact rate as `fractions.Fraction` and takes the same
standard type in `ImageSequenceSource.rate`. Native conversion checks positive
unsigned 32-bit components after standard fraction normalization.
Entity existence and production membership remain operation checks.
C++ content values after a move support destruction or reassignment; inspect
the destination. The input alternatives remain unvalidated request records.

Inspection properties derive the kind, members and sequence descriptor from the
content. They cannot disagree with a separately stored discriminator. Unknown
required native kinds fail decoding rather than constructing a partial value.

## Compatibility and standards impact

This changes binding source contracts, not persisted identities, schema, C
layouts or external standard mappings. Unknown namespaced roles retain their
spelling; their existing syntax and membership rules remain authoritative.
