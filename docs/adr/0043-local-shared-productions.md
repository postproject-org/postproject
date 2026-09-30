# ADR 0043: Local shared productions

- Status: Accepted
- Date: 2026-09-30

## Context

PostProject productions are SQLite files. Existing integrations usually choose
a production from one host project's location, which makes one process the
effective owner even though SQLite and the PostProject transaction contract
support several local writers. Cross-application use needs a narrower,
explicit contract so convenience discovery in one host does not become part
of the storage model.

## Decision

A shared production is one local `.pproj` file, on one machine, opened
independently by several processes. Each host receives or lets the user select
the same explicit production path. Directory nesting and another host's
project-root convention are not discovery protocols.

Each process uses the public PostProject API and normal SQLite-backed
transactions. A host records only facts it observes, identifies every commit
with its application name and version, and reads bounded revision pages to
discover other processes' durable changes. A local revision waiter may reduce
latency, but the revision cursor remains the correctness mechanism across
process restart.

Hosts adopt known media explicitly after locator or fingerprint lookup. They
may attach their own qualified external identifiers to the chosen asset, but
do not overwrite another host's identifier or infer that equal content means
equal logical identity. Hosts use base revisions for decisions that replace a
semantic fact and surface typed conflicts for explicit re-read and retry.

Every integration must continue to work alone and degrade to its upstream
behavior when PostProject or the selected production is unavailable. No host
reads another host's project format.

Productions on network filesystems, remote revision streams, a PostProject
service, server databases, and access-control boundaries are unsupported.
Their consistency and failure models are not extensions of this local-file
contract.

## Alternatives considered

- Inferring a production from the nearest common project directory was
  rejected because host directory conventions are unrelated and mutable.
- Making one host's project file authoritative was rejected because it would
  couple other hosts to a foreign format.
- Adding a daemon or server database was rejected because the demonstrated
  need is same-machine coordination and SQLite already supplies its physical
  transaction boundary.
- Treating network-mounted SQLite as local sharing was rejected because its
  locking and durability properties depend on the filesystem and deployment.

## Standards impact

No external media, metadata, identifier, provenance, or interchange standard
defines how applications select a local PostProject production. Existing
external identifiers and vocabulary values remain opaque and exact. This
decision adds no normative mapping or normalization.

## Migration implications

There is no schema or ABI migration. Existing sidecar discovery remains a host
convenience for single-host use. Hosts that participate in a shared workflow
add an explicit path selection and preserve their existing absent-production
fallback.

## Consequences

Independent applications can share durable production knowledge without a
service or knowledge of each other's project files. Operators must select the
same local file in each host, and integrations must retain revision cursors,
origin information, and conflict UI. This decision does not claim safe
operation across machines or network filesystems.
