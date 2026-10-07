# 0060: Bounded convenience reads

Status: accepted for the 0.7 development SDK.

## Decision

Complete-list convenience reads for assets, representations, resources and
locators use a single page with the existing maximum of 1000. A continuation
returns `Unsupported`, never a silently truncated list. Callers needing larger
collections use the existing scoped page operations. Missing parent objects
follow the page operation's `NotFound` contract.

The limit applies to Rust and native/binding callers through storage, including
resolver helpers. It does not reduce the domain's compound-member limit or
prevent paging larger productions. Retained views keep their existing snapshot
semantics; live convenience reads have the same consistency as one live page.

Producing/consuming activity helpers use the same 1000-item rule. Ancestor and
descendant helpers use the existing bounded traversal, at most 64 levels and
1000 visited representations. A continuation or traversal truncation returns
`Unsupported`; a partial graph is never presented as complete ancestry.
Complete revision-event lists likewise cap at 1000. Large atomic commits remain
valid; their immutable events are available through the existing event pages.

Materialized media, fingerprint, metadata, identifier, provenance, job and
revision rows have a private read budget
of 100000 rows and 64 MiB of stored payload, checked while streaming before
copying borrowed SQLite values or decoding. Pages include their lookahead row in that byte budget; choose
a smaller page after `Unsupported`. Domain and individual-value limits remain
separate. This bounds payload allocation without treating 64 MiB as an exact
total process-memory promise: decoded values and row bookkeeping have overhead.
Metadata property convenience queries use the ordinary 1000-item page rule.

## Migration and standards impact

Replace complete-list calls with paging when collections can exceed
1000. No signature, persisted identity, schema or external standard mapping
changes. Limits reject explicitly and preserve unknown external values.
