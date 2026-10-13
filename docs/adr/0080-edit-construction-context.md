# ADR 0080: Create edits with their decision and attribution

Status: Accepted

Ordinary native callers currently choose among additive, revision-only and
scoped constructors, then make another fallible call to set attribution.
A revision ID cannot represent a coherent decision at an empty journal.

Use `pp_production_begin_edit` with an optional scoped decision base and an
optional borrowed edit context. A null base creates an additive edit.
`pp_read_session_begin_edit` retains the view and accepts the same context.
Validate and copy origin/message before reserving an edit; invalid context
returns no handle and leaves the production available for another edit.

C++ exposes `production.edit(context)`, `production.edit(base, context)` and
`view.edit(context)`. Python keeps its existing `edit` keyword arguments.
Remove native revision-only constructors and public context setters. Rust's
backend transaction primitives remain available to backend implementers.

Commit is explicit and every attempt terminal. Retained and detached bases
protect the original decision, including an empty journal; additive edits do
not invent one. No-op commits return their own no-change receipt. Creation,
release and rollback publish nothing. Staged proposal preparation retains
the attribution selected at edit creation.

This changes native signatures and exports: ABI 55 and loader guards move
together. Schema 27, exchange protocol 1 and CLI JSON 1 are unchanged.
Standards-policy review: no identifier or metadata mapping changes.
