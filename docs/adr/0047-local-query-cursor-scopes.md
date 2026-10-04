# 0047: Local query cursor scope and lifetime

Status: accepted for development.

## Decision

Version `ppq2` cursor framing binds every bounded SQLite query to production
identity, query family, and existing filters/traversal bounds. Live queries
share a production scope across reopened handles. A read view adds a random
per-session nonce: its cursor works only in that retained view, even if another
session observes the same revision. Closing the view ends that continuation.

The scope is validated before decoding the continuation position. Cursors
remain bounded opaque strings, not locks, authentication capabilities, or
commit receipts. Live keyset pages may reflect intervening changes; snapshot
pages stay coherent. The CLI inspection closes its view and therefore emits
no resumable snapshot cursor.

## Migration and standards impact

Existing `ppq1` tokens must be discarded and their queries restarted. No stored
production facts or schema change. Snapshot tokens are local session state;
they are not portable checkpoints. Reviewed against the contributor standards
policy: no external identifier or interchange mapping changes.
