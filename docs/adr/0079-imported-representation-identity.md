# 0079: Import returns the actual representation identity

Status: accepted for development C ABI 54.

## Decision

Return the imported asset and original representation identities together.
Both already exist in the prepared import; obtaining them requires no query,
scan, extra fingerprinting or later-head read. They identify staged objects:
only a successful explicit commit makes those objects durable.

C adds an `out_representation_id` argument to both file and compound imports.
It may be null when the caller only needs the asset. Required outputs are
validated before staging; both supplied IDs are cleared on failure. These are
ordinary copied ID values, with no additional result owner or public layout.

C++ and Python return `ImportedMedia`, a copied record containing `asset_id`
and `representation_id`. This is a result, not another identity layer. Remove
the old asset-only return contract and migrate callers directly. Rust's
prepared import and CLI import output already expose both identities.

## Alternatives and impact

A representation-set owner could also expose all prepared facts, but forces
C callers to allocate, inspect and release an owner for two IDs. A new C
struct adds a layout without improving the pointer/output convention. Keep
bounded point reads for callers who need full representation/resource facts.

Native signature changes require ABI/soname 54 and matching bindings; symbol
names and public layouts remain unchanged. Package 0.9 remains unpublished.
Schema 27, exchange protocol 1 and stored identity/effect capture are unchanged.
CLI JSON 1 is unchanged; its existing import result supplies both IDs.

Standards-policy review: no recognition, vocabulary, identifier scheme,
normalization or domain meaning changes. Identical bytes remain independent
logical imports unless the host explicitly selects an existing identity.
