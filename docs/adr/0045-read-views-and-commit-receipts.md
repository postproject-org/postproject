# 0045: Coherent read views and atomic commit receipts

Status: accepted for development; public projections and migration are in progress.

## Decision

A read session owns a separate SQLite connection and a read transaction. Its
first read pins the production identity and revision in the same view used by
all subsequent domain reads. The session exposes only `ProductionRead`; it
cannot be upgraded to a writer. Returned domain values are owned. Dropping the
session closes its connection and releases the view.

Adapters may transfer that connection into a read-only SQLite facade to reuse
the checked projections of existing read operations. Its view and cursor scope
remain pinned; writes, nested sessions and live waiters reject immediately.
Public read sessions retain the detached base and original writer separately.

Requesting a read session enables WAL for that production so retained readers
permit concurrent writer progress. Ordinary create/open does not switch modes.
Keep the existing busy timeout, value limits and trusted-schema restrictions.
Retained views can prevent checkpoints from reclaiming WAL pages: callers
should detach their decision base and close the view before long host work.
No timeout silently converts a pinned view into live reads. Close every
connection before copying a production; copying only an active `.pproj` can
omit durable data in its WAL. Network filesystems remain unsupported.

An edit uses a fresh short write transaction and the read session's
production-scoped decision base. An absent revision means an empty journal;
it does not disable conflict checks. Validate production identity and revision
identity/sequence together. Existing semantic conflict keys protect mutated
facts, not every fact a caller read. This is not full read-set serializability.

Metadata replacement/removal (ADR 0052), root enabling/removal and locator
retirement require a decision base before reading or staging a mutation.
Even a request for the
root's existing state requires that base. Early rejection leaves the transaction
open and stages nothing. Root creation and metadata appends remain additive.
Other unbased mutation families remain under review during migration.

The commit path returns `CommitReceipt { production_id, revision }`, capturing
the new revision before the atomic SQLite commit. `revision: None` means the
transaction created no revision; it never attributes an existing head to the
caller. Return the receipt only after successful durable commit. Every commit
attempt is terminal, including errors preparing the journal or committing.
Rollback/drop never commits. Retry requires a new edit and an explicit decision.

Python's `Transaction` and read-bound `Edit` contexts both require explicit
`commit()`. Every uncommitted exit rolls back, including normal exit. Explicit
close or terminal failure inside the context never triggers another mutation.
The 0.7 series removes the old transaction context's implicit commit convention.

## Migration and standards impact

Introduce focused APIs alongside existing experimental operations while the
consumer migration proceeds. The final candidate must migrate ordinary callers
and explicitly resolve remaining legacy entry points. Existing 0.6 tags and
the C++ Result propagation promise remain intact. ABI/version decisions are
made with the public projections; this storage change adds no schema migration
and preserves persisted UUIDs and external facts.

Reviewed against `docs/src/contributors/standards-policy.md`: no external
identifier, vocabulary, timecode or interchange mapping changes.

References: [SQLite isolation](https://www.sqlite.org/isolation.html) and
[WAL](https://www.sqlite.org/wal.html). The read transaction must execute its
first read before returning; `BEGIN` alone is not the pinned-view contract.
