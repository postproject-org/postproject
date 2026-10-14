# 0084: Discover worker and exchange values in explicit Python modules

Status: accepted for development Python 0.9.

The package root introduces ordinary production, media, metadata and coherent
edit/read values. Move worker requests, leases, states and job events to
`postproject.jobs`. Move portable proposal commands/builders, outcomes,
replication positions and exchange limits/errors to `postproject.exchange`.
Remove their root exports and migrate all current callers; no aliases remain.

The modules expose the same values and owners, without another facade or
runtime identity hierarchy. Ordinary workflows need neither module. Work and
exchange operations on productions/edits remain available and documented in
their respective advanced guides.

Staged-edit proposal preparation captures already prepared native effects;
the command builder expresses deferred intent. Keep both workflows and their
distinct lifetimes. Notification cursors remain separate from replication
positions. No lease renewal/release or other write occurs on finalization.

This organization accompanies the simplified import, lookup, relink, output
and metadata tasks; moving names alone is not workflow simplification. C ABI
57, schema 27 and protocol framing 1 are unchanged. Standards-policy review:
no domain or standards mapping changes.
