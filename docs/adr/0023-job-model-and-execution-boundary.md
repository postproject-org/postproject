# ADR 0023: Job model and execution boundary

- Status: Accepted; regeneration requests amended on 2026-09-28
- Date: 2026-09-24

## Context

Applications need to share that production work is wanted, claimed, completed,
or failed without requiring PostProject to own a scheduler or a host's task
system. Activities cannot represent this lifecycle: ADR 0005 defines an
activity as a complete fact, and a failed or pending operation did not produce
that fact.

Claims must be safe when two processes use the same SQLite production. Tests
must not depend on wall-clock timing, and completing a job must not leave an
output representation without its activity or mark a job successful before
those facts are durable.

## Decision

A job is persisted production knowledge with:

- a stable job ID and open-world, namespaced kind;
- ordered input representation references;
- typed parameter metadata targeting the job;
- a requested output naming an asset, representation kind, and optional
  logical target-root name;
- lifecycle state and, when applicable, claim, failure, or completion detail.

The lifecycle is:

```text
requested -> claimed -> succeeded
                     -> failed
                     -> cancelled
claimed   -> requested  (release)
requested -> cancelled
```

An expired claim may be replaced atomically by another claimant. Expiry is
tested against a caller-supplied `now`; storage never reads the wall clock.
Claim and renewal require a future lease expiry.

Each successful claim receives a random claim-token UUID. Renew, release,
complete, and fail require that token. Worker tool and agent identity remain
inspectable attribution, while the token prevents two processes using the same
descriptive identity from completing each other's claims. Claim tokens are
capabilities scoped to one production and are not authentication credentials.

Every lifecycle mutation occurs inside an explicit production transaction and
emits a semantic revision event. SQLite's immediate write transaction provides
the one-machine serialization boundary: two transactions cannot both replace
the same requested or expired claim.

Completion is one transaction containing the output representation and
resources, the completed activity with storage-captured edge snapshots, and
the transition to `succeeded` referencing that activity and output. Storage
validates the claim token, requested asset and representation kind, and the
activity's job inputs/output. Any failure rolls back all of it. Failure records
a bounded diagnostic and no activity. Cancellation is allowed from requested
or claimed; release returns a claimed job to requested.

PostProject exposes the protocol but does not select priorities, distribute
work, spawn processes, retry failures, download tools, or run a background
scheduler. Regeneration planning is a read operation and never enqueues jobs;
requesting each returned plan remains an explicit caller transaction. A plan
takes its inputs from the producing activity and its parameters from the
activity's metadata. When that activity completed a job, the plan repeats the
job's kind and logical target root, so the regenerated work reaches the same
workers and the same destination. An activity recorded without a job yields the
activity's kind and no target root. Schema 14 indexes jobs by their completion
activity, so this lookup reads one index entry per artifact.

## Alternatives considered

Representing pending work as incomplete activities was rejected because it
would weaken the meaning and invariants of all existing provenance. Identifying
a claim only by worker name was rejected because names are descriptive and not
unique. Implicit current-time reads were rejected because they make storage
behavior and tests timing-dependent. Completing output, activity, and job in
separate transactions was rejected because crashes would expose contradictory
production facts.

Deriving a plan's kind and target root from the activity alone was rejected
too. An activity names what the worker did, which can differ from the requested
kind, and it records no destination, so the regenerated job could go unclaimed
or be written elsewhere.

A daemon, network queue, and distributed lock service were rejected as product
scope. Hosts remain free to mirror jobs into their own task systems and use the
claim protocol as the production-knowledge boundary.

## Standards impact

Jobs are not exported as W3C PROV activities. Only successful completion
creates the activity fact that maps conceptually to PROV. Claim tokens, leases,
and failure diagnostics are PostProject workflow concepts and introduce no
canonical external scheme, vocabulary mapping, cardinality, or normalization
rule. Existing identifiers and unknown typed metadata continue to round-trip
without registry knowledge.

## Consequences

Multiple local processes can coordinate production work without a service, and
all time-sensitive behavior is deterministic under an injected clock. Hosts
can use their existing schedulers while sharing portable requested/failed work.
The model adds a new metadata/host-binding object kind, schema tables, semantic
events, and public API/ABI surface. Claim-token holders must protect the token
for the duration of a claim, and a lost token requires waiting for expiry or an
explicit administrative cancellation.
