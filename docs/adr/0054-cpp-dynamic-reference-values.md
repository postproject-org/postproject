# 0054: C++ dynamic reference values

Status: accepted for development.

## Decision

`ObjectRef` owns a private `std::variant` of production, asset,
representation, resource, activity and job identities. Typed factories retain
the selected identity kind. `value()` borrows a const variant for standard
`std::get_if` inspection; `kind()` derives the discriminator. There are no
independently writable kind/UUID fields or aggregate constructor.

Kind-specific projections return `Result` on a mismatched alternative.
`asUuid()` and checked `fromUuid(kind, uuid)` provide explicit interchange;
unknown discriminators return errors. Native decoding propagates those errors
through `Result`, including conflict, metadata, dependency and artifact paths.
Both exception-oriented and exception-free callers use the same projection.

Identity types and reference alternatives cannot establish entity existence,
production membership or lease authority. Store checks remain authoritative;
explicitly retagging interchange bytes does not establish the original kind.

## Migration and standards impact

Replace `.kind` with `.kind()`, and `.id` with a typed projection or explicit
`.asUuid()`. Replace kind/UUID aggregate construction with a typed factory;
use checked `fromUuid` only at interchange boundaries. C11 inputs, C ABI 47,
schema 17, Python references and persisted binding text are unchanged.
Reviewed against the standards policy: this changes C++ construction and
projection only, without external identifier, vocabulary or interchange rules.
