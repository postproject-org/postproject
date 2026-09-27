# ADR 0031: Resolution scope, budgets, batches, and cancellation

- Status: Accepted
- Date: 2026-09-27

## Context

Resolution searched only logical media roots mapped for this machine (ADR
0017). It resolved one asset per call, walked every root again for each
resource, and stopped after 100,000 entries counted across all roots. Reaching
that limit discarded every candidate already found and turned the result into
an error. This contradicts ADR 0017: one root must not prevent discovery
beneath another. The C ABI exposed none of the resolver's options. It had no
content-verification tier, no limits, and no way to stop a long scan.

Integrations showed that these are general gaps:

- **Hosts without logical roots.** Many editors, compositing tools, and scripts
  know only absolute paths and a project folder. To say "look near where it
  was", they had to invent roots named from path digests. Those labels were
  machine-local data stored in the production.
- **Many offline assets at once.** Opening a project with many offline clips,
  or answering an OpenAssetIO batch, rescanned the same directories once per
  resource.
- **Interactive hosts.** A host resolving from a UI thread could not stop a
  scan of network storage.

## Decision

**Search scope.** A resolution call searches a scope with two parts. Neither is
recorded in the production.

- *Mapped logical roots*: the production's enabled roots, located through this
  machine's mappings (ADR 0017).
- *Search directories*: unnamed, machine-local directories, searched after the
  roots.

A candidate found under a root carries that root's name, so confirmation can
record it portably. A candidate found only in a search directory carries none.
The caller confirms it with a plain locator, or under a root of its own choice.

**Per-directory budgets.** Each searched directory has its own depth and entry
budget. A directory that exceeds its budget is searched only partially. It adds
`SearchTruncated` evidence naming the directory, and resolution continues with
the other directories and the candidates found so far. A scan limit never turns
a result into an error on its own. An unusable scope, such as one root mapped
twice, still does.

**Batches with one scan.** One call resolves many resources. Known locators are
checked first. Every directory is then walked at most once, lazily, into an
index of regular files with their sizes. Each resource filters that index by
size and name before hashing any candidate. Sequence discovery looks only at
directories that contain the first expected frame. On the C ABI, resolution
takes an array of assets. A single asset is a batch of one.

**Options object.** The C ABI passes the scope, the verification tier, the
limits, and an optional cancellation token in an opaque, growable options
object. A NULL options pointer means the defaults.

**Cancellation.** A cancellation token is a shared flag that any thread may set.
Resolution checks it while walking directories, before each resource, and
before hashing each candidate. It then fails with a dedicated cancelled error.
Nothing is committed, because resolution is read-only. The token is generic, so
other long operations can adopt it.

## Alternatives considered

- **Callbacks for progress and cancellation.** The C ABI has no callbacks on
  synchronous paths. A polled token needs no re-entrancy, thread-affinity, or
  unwinding rules, and matches the revision waiter's cancel function.
- **A persistent discovery index.** ADR 0018 keeps discovery indexes out of the
  production. A per-call index gives the batch gain without staleness.
- **Registering search directories as roots.** Roots are portable production
  knowledge. A project folder on one laptop is not.

## Standards impact

None. Locators remain `file:` URIs (ADR 0030). Search directories never reach
persisted data.

## Consequences

ADR 0017's rule that one root cannot hide results under another now also covers
scan limits. Hosts without roots resolve relocated media without inventing
roots. Resolving N offline resources costs one scan plus N filtered lookups,
instead of N scans. Memory grows with the number of regular files indexed per
call, and the per-directory budget bounds it. The C, C++, and Python resolution
functions change shape: they take several assets and an options object, and a
candidate exposes its root name.
