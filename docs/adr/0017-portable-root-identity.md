# ADR 0017: Portable root identity

- Status: Accepted
- Date: 2026-09-23

## Context

A media root currently persists an absolute URI in the production database.
That URI is useful on the machine that created it but becomes meaningless when a
collaborator mounts the same storage elsewhere or the production moves between a
workstation and a laptop. An absolute machine path is not portable production
knowledge.

Content fingerprints can recognize media after it is found, but without a
portable root identity the resolver does not know where to search. Treating an
unmapped root as ordinary missing media also hides the configuration problem.

## Decision

A production stores a unique logical name for each media root. A caller supplies
machine-local mappings from those names to absolute filesystem paths when it
scans or resolves media. Mappings are runtime input or live in a configuration
file outside the `.pproj` database; they are never committed as production
knowledge.

An enabled root can be in one of three operational states:

- mapped and reachable;
- mapped but currently unreachable; or
- unmapped on this machine.

The resolver reports these states independently and continues through every
usable root. One unreadable or unmapped root cannot prevent discovery beneath a
different root. ADR 0031 extends this to scan limits and adds unnamed,
machine-local search directories for hosts that have no roots.

Schema migration preserves an existing absolute root URI as a legacy fallback
mapping. New roots have only a logical name. The fallback prevents data loss and
keeps an old production usable while the caller establishes explicit mappings;
it is not the identity of the migrated root and may be retired after migration.

Root names are bounded UTF-8 identifiers intended for people and configuration
files. They are unique within one production and are not global namespaces,
volume serial numbers, or storage-provider identifiers.

## Alternatives considered

- **Volume or filesystem identity.** It does not cover copied media, network
  mounts, containers, or platform differences consistently.
- **Several absolute URIs per root.** This leaks each collaborator's machine
  state into the shared production and creates synchronization conflicts.
- **Environment-variable expansion in persisted URIs.** It makes interpretation
  process-dependent and embeds a configuration language in production data.

## Consequences

Every resolver-facing integration must provide or deliberately omit a root map.
An omitted mapping is diagnosable rather than an offline result. Legacy files
open without losing their paths, while newly created files no longer depend on
one machine's mount layout. Remote storage remains deferred; this decision only
defines identity and mapping, not transport adapters.
