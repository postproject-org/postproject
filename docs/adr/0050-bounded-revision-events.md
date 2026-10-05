# 0050: Bounded revision-event reads

Status: accepted for development.

## Decision

Offer revision events as position-ordered pages using `QueryPageRequest` and
its existing 1–1000 item bound. Keyset cursors include the revision filter and
ADR 0047's production/read-view scope. Read sessions expose this bounded
operation; copied event values survive session closure. Unknown or future
revisions return `NotFound`, including revisions outside a retained view.

The existing whole-revision convenience remains subject to the API safety
audit. It is not the implementation of paging: SQL applies the page limit
before decoding or allocating event projections.

## Migration and standards impact

Callers processing large transactions continue until `next_cursor` is absent.
No persisted data, schema, event meaning or external standards mapping changes.
Reviewed against the contributor standards policy; this changes query resource
bounds and lifetime only.
