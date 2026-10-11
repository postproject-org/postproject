# 0070: One Python edit type covers staged writes

Status: accepted for development.

## Decision

Replace `Production.transaction()` and the `Transaction` class with
`Production.edit(base=None, *, origin=None, message=None)` and `Edit`.
Omitting the base stages additive writes. A detached `DecisionBase` carries
the production and revision observed together; a revision ID alone no longer
starts a Python edit.

Retain `ReadSession.edit(*, origin=None, message=None)` because it preserves
the exact pinned knowledge used by automatic content observations. This
owner-specific constructor has a distinct lifetime contract; detaching a base
does not preserve that snapshot. Both constructors return the same `Edit` and
accept the same context keywords. Failed context validation releases the
new handle and its write guard.

Commit stays explicit and terminal, including failures. Uncommitted context
exit rolls back, no-op commits create no revision, and callers use their own
receipt. Conflict handling requires a new read, decision and edit.

## Migration and standards impact

Use `production.edit()` instead of `production.transaction()`. Capture
`view.decision_base` while reading instead of passing `base_revision=`.
Replace `Transaction` type annotations with `Edit`; no alias is provided.

Reviewed against the contributor standards policy: this changes Python
construction and source names only. Storage, C ABI, metadata, domain meaning
and portable exchange formats remain unchanged. No external mapping changes.
