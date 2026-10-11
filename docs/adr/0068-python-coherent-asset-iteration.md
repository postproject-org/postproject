# 0068: Python asset scans belong to a retained view

Status: accepted for development.

## Decision

Replace the eager `Production.assets` collection with
`ReadSession.assets(page_size=100)`. The caller explicitly opens and closes
the pinned view. Iteration loads at most one asset page, using the existing
session-scoped cursor. Page size is checked at iterator creation; every
advance through buffered values checks the session lifetime. Exhausted
iterators remain exhausted. Copied asset values survive session closure.

Iterator closure never closes its session. Errors retain their existing
structured exception data. The iterator adds no total limit or implicit
cancellation/I/O; existing page APIs remain available for manual continuation.
The view retains its SQLite snapshot until the caller closes it, so callers
should close it before long host work. It is not an indefinitely pinned
connection acquired by a hidden generator.

Membership becomes the typed point query `Production.asset_exists(asset_id)`.
Remove the old collection's iteration, length and membership implementations;
do not provide compatibility aliases. Callers that need a complete list
explicitly collect the session iterator.

## Migration and standards impact

Replace `tuple(production.assets)` with a scan inside `read_session()` and
`id in production.assets` with `production.asset_exists(id)`. Live point
queries and pinned scans deliberately have different consistency boundaries.

Reviewed against the contributor standards policy: this changes only Python
source/lifetime semantics. UUIDs, domain meaning, storage, C ABI, cursor
framing and portable exchange formats remain unchanged. No external standards
mapping or normalization is introduced.
