# ADR 0041: Known-media adoption

- Status: Accepted
- Date: 2026-09-30

## Context

Two applications can encounter the same stored media while sharing one
production. Before this decision, a host could inspect an already-known asset
but could not start with locator or content evidence and discover the owning
resource, representation, and asset. Importing unconditionally would create
avoidable duplicate logical assets. Equating paths or bytes with logical asset
identity, however, would be incorrect: paths can be reused, and identical bytes
can intentionally belong to distinct production concepts.

Image-sequence directories introduce another ambiguity. Several sequences can
occupy one directory, and copies of one sequence can use different file-name
patterns. Historical locators and superseded fingerprints are unsafe defaults
because they do not describe the current effective storage state.

## Decision

PostProject provides bounded, read-only known-media queries. Every match is an
explicit owning path containing the matching resource, its parent
representation, and that representation's asset. Results use stable keyset
pagination and return every match; the library never chooses a candidate.

Locator lookup accepts a canonical locator identity. A single-file identity is
its normalized absolute URI with no sequence naming. An image-sequence identity
is the normalized directory URI plus its exact prefix, suffix, and padding.
Directory-only lookup does not match a sequence. Normal locator lookup reads
only current locator rows; retired locator history is not searched.

Content lookup accepts the resource fingerprint algorithm, version, and opaque
value. Algorithm identifiers are open to foreign hosts. Normal content lookup
reads only the current effective resource-fingerprint table and never searches
superseded observations. Content equality produces candidates; it does not
establish logical identity.

Adoption remains explicit application behavior. A host may attach its own
qualified external identifier to a selected asset and may confirm another
locator through ordinary mutation APIs. Lookup itself does not import, merge,
relink, attach, or otherwise mutate anything. If two hosts both observe no
match before either commits a first import, both imports may succeed as
distinct logical assets. Their later lookups return the ambiguity.

Schema 16 adds indexes beginning with locator URI and with resource fingerprint
algorithm, version, and value. The authoritative rows remain unchanged; the
indexes make candidate selection proportional to the matching rows and page
size rather than the production's total media count.

## Alternatives considered

- Global uniqueness for locator or fingerprint values was rejected because
  neither storage location nor content equality proves logical identity.
- Automatic adoption of a single match was rejected because host policy and
  qualified application identity must remain explicit.
- Searching historical locator and fingerprint observations by default was
  rejected because retired paths can be reused and superseded content is not
  the resource's current state.
- Matching image sequences by directory alone was rejected because it conflates
  independent naming layouts in one directory.
- Returning only asset IDs was rejected because callers need the exact matching
  storage resource and parent representation to evaluate and explain adoption.

## Standards impact

No external media, metadata, identifier, provenance, or interchange standard
defines logical adoption for this application-neutral production model. The
query preserves foreign fingerprint domains and opaque values verbatim. It
does not assert that two fingerprints from different algorithms are
equivalent, and it introduces no normative standards mapping.

## Migration implications

Opening a schema 15 production creates two indexes and advances it to schema
16 without rewriting or discarding locator, fingerprint, or ownership data.
Older PostProject builds reject schema 16 as newer than supported. The public
surface is additive, but the C ABI version increases when the queries are
projected through C, C++, and Python.

## Consequences

Applications can avoid duplicate imports when they deliberately recognize
known media, while ambiguity remains visible and harmless. Current-state
semantics are predictable across hosts. A separate historical-search API would
need an explicit future contract. Concurrent first import deliberately remains
a race that may create distinct assets; PostProject does not become a global
content-deduplication service.
