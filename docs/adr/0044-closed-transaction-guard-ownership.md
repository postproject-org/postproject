# ADR 0044: Closed transaction guard ownership

- Status: Accepted
- Date: 2026-10-04

## Context

The installed C and C++ consumers reproduce a lifecycle defect in release
0.5: close transaction A, begin B, release A, then begin C. C incorrectly
succeeds while B remains open. Commit, rollback, and failed commit all expose
the defect. C++ move and destruction expose the same C handle lifetime.

## Decision

Only an open transaction owns the production's open-transaction guard.
Commit and rollback relinquish that ownership; destruction clears the guard
only when the transaction is still open. Closed handles may remain allocated
while a newer transaction is open. Failed commit still closes the transaction.

This restores the documented one-open-transaction contract without changing
C declarations, symbols, ownership operations, projections, or schema. No
ABI bump or consumer migration is required. Public C and C++ installed tests
cover all three terminal paths and opening again after releasing B.

## Standards impact

This is local handle bookkeeping. It adds no identifier, vocabulary, mapping,
normalization, or persisted domain fact under the standards policy.
