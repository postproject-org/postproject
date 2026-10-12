# 0076: C++ target-wide metadata reads

Status: accepted for development 0.9.

## Decision

Provide `metadata(ObjectRef)` on C++ `Production` and `ReadSession`, matching
the existing Rust, C and Python operation. Return copied
`std::vector<MetadataAssertion>` values through `Result`; the native set is
released before returning. Copies survive view/production disposal.

Retain assertion order, repeated properties and every typed value. Do not
collapse assertions into a map or infer types from vocabulary definitions.
The view overload reads its pinned knowledge; the production overload reads
current knowledge. Existing bounded property queries remain appropriate for
cross-target searches. Errors propagate through both established C++ styles,
including builds without exceptions.

## Impact

This closes a projection gap without changing C symbols/layouts, schema,
protocol or package versions. Reviewed against the standards policy: no
vocabulary mapping, normalization or domain meaning changes.
