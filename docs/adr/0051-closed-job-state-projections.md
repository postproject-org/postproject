# 0051: Closed job-state projections

Status: accepted for development.

## Decision

Project Rust's `JobState` as a C++ `JobStatus` variant and a Python union of
immutable alternatives: requested, claim detail, completion detail, failure
diagnostic or cancelled. A job has one status value; it cannot simultaneously
carry a claim, completion and failure. Category-only enums remain useful query
filters. Decoding rejects unknown required alternatives and missing payloads.

C++ callers use `std::get_if` or `std::visit`. `stateKind()` returns the category
or no value if a caller's throwing variant assignment left it valueless.
Python category/payload properties are read-only views derived from the status;
construction validates the alternative and copies input collections.

The C API provides checked claim, completion and failure accessors. Wrong-state
requests return `InvalidArgument`; absent indices return `NotFound`. Failure
outputs are zero or null. Strings borrow their owning job set. The flattened
C job view remains a transitional projection in the open safety audit; its kind
must be checked before reading state-specific fields. Claim
capabilities and authoritative lease time are separate, still-open contracts;
claim detail in this slice retains the existing fields.

## Migration and standards impact

C++ replaces independent `state`, `claim`, `completion` and diagnostic fields
with `status`. Python replaces these constructor arguments with `status` while
retaining derived inspection properties. No persistence, schema, lifecycle
meaning or external standards mapping changes. Reviewed against the contributor
standards policy; this changes public projections only.
