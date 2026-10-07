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

## Migration and standards impact

Replace complete-list calls with page iteration when collections can exceed
1000. No signature, persisted identity, schema or external standard mapping
changes. Limits reject explicitly and preserve unknown external values.
