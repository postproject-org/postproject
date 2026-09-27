# ADR 0029: Content fingerprints and observation for hosts

- Status: Accepted
- Date: 2026-09-27

## Context

ADR 0021 made a fingerprint the current observation of an object's content. It
also made the media crate the only implementation of the structure-aware
representation fingerprint. Recording a new resource value marks every owning
representation for recomputation until the recomputed value is recorded too.

A native host could record fingerprints but could not compute one. The C ABI
accepted caller-supplied bytes and offered no function that fingerprints a path
or recomputes a representation. Only Rust callers and the CLI could observe
changed content correctly. Integrations therefore invented their own resource
and representation "fingerprints" in private domains, which the resolver and
artifact evaluation cannot use.

Three kinds of integration need this:
- **Job workers** such as proxy renderers, transcoders, and render-farm tasks
  must record the fingerprint of an output, or of a source replaced in place,
  so that artifacts evaluate as stale.
- **Hosts** that relink, or check media before use, must compare a file with
  what the production recorded, without importing the file again.
- **Editors** replace media in place and must record the change as one revision.

## Decision

PostProject offers three read-or-stage operations on every surface. They are
backed by one media-crate implementation.

1. **Compute a file fingerprint.**
   - Rust: `fingerprint_file`. C: `pp_fingerprint_file`.
   - It returns the value PostProject would record for a file on import, with
     its algorithm, version, and bytes.
   - It needs no production and records nothing.
2. **Verify a stored resource.**
   - Rust: `verify_resource_content`. C: `pp_production_verify_resource`.
   - It compares present content at a path with the resource's stored
     fingerprints and reports *matches*, *differs*, or *not comparable*.
   - Only domains PostProject computes are compared, as in ADR 0028. Changed
     content that selects another size-dependent file domain differs.
   - Verification is read-only.
3. **Observe a resource's content.**
   - Rust: `observe_resource_content`. C:
     `pp_transaction_observe_resource_content`.
   - It fingerprints the present content of a file or image-sequence resource,
     then recomputes every representation that uses the resource, with the new
     value in place of the stored one in that domain.
   - The transaction stages the resource and representation values. Commit
     records them in one revision, so no representation is left pending.
   - Hashing happens when the operation is staged, outside the production lock.

An explicit transaction call is still the only way to record, as ADR 0021
requires. Observation is a domain operation, "this path now realizes this
resource", not row CRUD. The caller never handles the representation algorithm.
Several observations staged in one transaction build on each other, so
observing two members of one representation yields the aggregate over both new
values.

`pp_transaction_record_resource_fingerprint` and its representation counterpart
remain. They record foreign domains (ADR 0028) and values computed elsewhere,
for example by a worker process without access to the production.

## Alternatives considered

- **Compute only, and let hosts record representation fingerprints.** This
  exposes the structure-aware algorithm to every caller and invites the private
  aggregates the examples already had to invent.
- **Recompute representation fingerprints inside the storage adapter.** ADR 0021
  keeps storage free of the media algorithm deliberately. Storage marks the
  representation for recomputation, and the media crate computes.
- **A single `pp_fingerprint_set` for several domains.** A file has exactly one
  PostProject domain, selected by size. One value keeps the result obvious.
  Further domains can add operations when they exist.

## Standards impact

None. The fingerprint domains are PostProject-defined (ADR 0015, ADR 0021).

## Consequences

Native hosts and workers can keep content identity current without Rust. The
CLI `media fingerprint` takes only a resource and a path, and a new
`media verify-content` command reports the verification verdict. A resource
used by more than 1,000 representations cannot be observed in one operation, in
line with the bounded-query policy of ADR 0026.
