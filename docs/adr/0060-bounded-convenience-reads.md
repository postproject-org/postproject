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
Resource pages share their budget with all owned fingerprints. Activity pages
and complete activity reads likewise share it across edges and snapshots;
component queries cannot each allocate a fresh budget for the same result.
Known-media pages share it across matched assets, representations and resources;
job pages and captured dependency paths share it with their owned child rows.
Artifact explanations and their traversal-cache copies also have finite row and
payload budgets, including duplicated fingerprints and authored path strings.
An oversized explanation returns `Unsupported` without modifying knowledge.
Native representation handles share one additional budget across every
representation, member, sparse frame, resource, locator and fingerprint they
retain. Enrichment cannot multiply the per-query storage budget; choose a
smaller representation page after `Unsupported`.
Media resolution bounds the combined resource/search request, shared directory
index and retained result payloads independently at 100000 items and 64 MiB.
Depth is 1..64. Directory traversal streams unsorted entries with at most 65
open ancestors: sorting or closing an ancestor would buffer an entire directory
before the entry budget. Returned candidates retain their deterministic order;
a truncated search can inspect only part of a directory.
Per-directory truncation evidence remains unchanged; aggregate overflow fails
explicitly with `Unsupported` instead of silently discarding candidates.
The native stored-knowledge snapshot shares a budget across representations,
resources, locators, fingerprints and roots before holding the combined work.
Native resolution sets also share their copied-payload budget across all
representations, resources, candidates, evidence and missing-frame issues.
Check the complete borrowed result before creating native string copies;
large requests return `Unsupported` and can be split into smaller asset groups.
Regeneration plans likewise share their budget across all copied job inputs
and producer parameters; a bounded parameter set cannot be copied once per
artifact without an aggregate limit.

CLI representation, dependency and metadata JSON files are limited to 64 MiB
before parsing. Array decoders enforce the existing domain item limits while
collecting. Oversized or invalid JSON returns `InvalidArgument` before opening
the production; no values are silently truncated.

## Migration and standards impact

Replace complete-list calls with paging when collections can exceed
1000. No signature, persisted identity, schema or external standard mapping
changes. Limits reject explicitly and preserve unknown external values.
