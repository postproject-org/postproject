# 0068: Python asset scans belong to a retained view

Status: accepted for development.

## Decision

Replace the eager `Production.assets` collection with
`ReadSession.assets(page_size=100)`. The caller explicitly opens and closes
the pinned view. Traversal loads at most one asset page, using the existing
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
Remove the old collection's traversal, length and membership implementations;
do not provide compatibility aliases. Callers that need a complete list
explicitly collect the session iterator.

`ReadSession.revisions(after=0, page_size=100)` applies the same lifetime and
page-size rules to ascending revision summaries. It is a finite scan of the
pinned history, rather than a live subscription. The iterator advances using
the last copied sequence, retains native errors and adds no total cap. Manual
`changes_since` pages and revision waiters remain available.

Rust `ProductionReadSession::assets(page_size)` and
`revisions(after, page_size)` borrow the retained view and yield fallible,
owned values. Rust prevents dropping the view while either iterator is in
use. They validate size eagerly, fetch lazily and end after a single backend
error. Low-level pages remain available. No new C ABI operation is needed.

## Migration and standards impact

Replace `tuple(production.assets)` with a scan inside `read_session()` and
`id in production.assets` with `production.asset_exists(id)`. Live point
queries and pinned scans deliberately have different consistency boundaries.

Reviewed against the contributor standards policy: this changes Python and Rust
source/lifetime semantics. UUIDs, domain meaning, storage, C ABI, cursor
framing and portable exchange formats remain unchanged. No external standards
mapping or normalization is introduced.
