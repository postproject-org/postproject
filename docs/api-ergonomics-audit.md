# API ergonomics audit

Baseline: published `v0.8.0-alpha.1`, `7e097bd`, inspected 2026-10-11.
Python is `0.8.0a1`, C ABI 52 has 349 exports and 36 layouts, schema is 27,
and exchange protocol is major 1. The local 0.9 development SDK now uses
Python `0.9.0a1`, ABI 53, 344 exports and 38 layouts. Schema and protocol
remain unchanged. This record distinguishes implemented changes from pending
qualification; it does not announce a release.

## Reconciled starting point

The 0.8 release has 18 assets. All six release jobs, sixteen tag CI jobs and
both documentation jobs passed. The final publication checkpoint supersedes
the earlier unpublished snapshot. Historical artifacts remain unchanged.

The [repository inventory](api-ergonomics-repositories.json) records all 13
active repositories, starting commits and unrelated untracked builds. All
start on `main`. The unused Shotcut checkout has no origin and is outside the
active inventory. No additional repository is needed.

## Five workflows and measured changes

Costs count visible setup, rather than fixture creation or assertions. The
[0.9 migration guide](src/integrators/api-migration-0.9.md) links runnable
recipes. Its Python before-program passes against the downloaded published
0.8 platform wheel; the after-program passes against the local ABI 53 SDK.

| Task and audited caller | Baseline cost | Implemented change | Preserved contract |
|---|---|---|---|
| Import: Python host, C guide and OBS registration | Python has separate additive transaction and decision edit interfaces; C file staging uses creation, staging and release calls | One Python `Edit` type; C file staging uses one checked call and no source owner | Explicit commit, rollback, atomic effects and own receipt |
| Adopt: Blender and OBS handoffs, OpenAssetIO manager | Eager whole-asset copy for membership/fixture checks; explicit page cursor loop for complete scans | Typed point membership and lazy view-owned Python/Rust scans | Existing identity, complete lookup and explicit ambiguous selection |
| Resolve moved media: Natron, Kdenlive and Ardour | One empty factory plus a separately checked call for every supplied setting | One checked C/C++ configuration factory using ordinary arrays/containers/paths | Immediate atomic validation, retained cancellation and explicit confirmation |
| Derived output: Blender and Manager publishing | Two metadata constructors for each ordinary key/value; extra revision-context object in scoped edits | Native property pairs/scalars and consistent Python edit context keywords | Exact kinds, ordered/repeated knowledge and atomic output/provenance |
| Revisions: coherent-read recipes and Rust callers | Caller manages sequence, pages and exhaustion; Rust/C++ duplicate receipt methods | Finite lazy view scans and one receipt-returning commit per surface | Pinned history, bounded pages, visible errors, terminal commit and no-op receipt |

The CLI removes one of its two decision-base options: `inspect` supplies the
single production-scoped token, including an empty journal. Help exposes
common tasks. JSON format 1 and exit categories retain their meanings.

## Canonical inputs and ownership

Python keeps ordinary UUID runtime values with distinct static ID hints.
Generic targets retain explicit `AssetRef`/`RepresentationRef` kinds: a UUID
cannot recover a `NewType` tag at runtime. Native metadata inputs pass through
one checked conversion. Typed property/value classes remain necessary for
lossless reads and ambiguous domain meanings, rather than compatibility.
Unknown vocabularies, nested/repeated fields and all explicit kinds remain.

Python scans check the caller-owned view on every advance, even with buffered
values. Rust scans borrow it. Both validate page size eagerly, fetch lazily,
retain errors and add no total cap. Copied values survive view disposal. Manual
pages remain useful for explicit continuation. Scans acquire no hidden view.

C point reads retain one owning result set with bounded copied facts; their
accessors borrow only from that named owner. C++ copies the checked domain
value through RAII. Reusing the result family avoids a second ownership shape
for one-element reads. Native pilots predominantly query known identities or
bounded candidate sets; no additional C++ scan owner is justified by those
callers. Full scans remain possible through explicit retained-view pages.

Single files accept native paths. Compound sources keep their meaningful
sequence, ordered-part and package descriptions. Configurations copy complete
inputs and retain cancellation state; failed construction leaves no options.
Commit has one result-bearing operation. A missing C receipt output rejects
before an attempt; every actual commit attempt remains terminal.

Maintained Python consumers have no independent durable-worker renewal loop
needing an atomic helper. The reference CLI worker owns its existing explicit
short renewal transaction. Keep staged lease composition and authority-time,
fencing, scope and output guards; do not add another worker interface without
a demonstrated caller.

## Capability and qualification boundaries

Focused checks cover metadata equivalence/replay, malformed input, own/no-op
receipts, guard cleanup, view lifetime, concurrent writers, corrupt pages,
atomic options and retained cancellation. Detached bases, semantic conflicts,
lease fencing, proposal idempotency, passive replay and complete structured
errors remain part of full qualification. Removing old names is not proof of
those semantics.

All consumers have initial migration commits. Current ABI 53 installed/native,
Python/typing, real-host and both-handoff gates are being rerun. Distribution
packets, clean installs and full candidate qualification remain pending.
The local site has reviewed desktop/mobile light/dark previews. Publication,
deployment, licensed Nuke execution and upstream acceptance are separate.
