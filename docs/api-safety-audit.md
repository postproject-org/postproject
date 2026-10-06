# API safety audit

Development audit against released `0.6.0-alpha.1`
(`a413227b9dd048f744fa7393d8927891ee93d5e2`, ABI 37, schema 17).
This is a working inventory, not a release acceptance report. An open row
does not claim that its replacement contract has been implemented.

The authoritative C entry-point inventory is
[`expected-symbols.txt`](../tests/abi/expected-symbols.txt). All declarations
in the linked source modules, including constructors, setters and decoders,
belong to the rows below. The compatibility-family manifest covers evidence
subsets; it is not a complete safety inventory.

| Family and source | Existing risk / required invariant | Callers / evidence | Status |
|---|---|---|---|
| Identity: core `id.rs`, C header, C++ `Uuid`, Python `_model.py` | C/C++ generic UUIDs permit wrong kinds; The released Python binding uses runtime ID subclasses. Preserve wire IDs; distinguish kinds statically and validate references in storage. | All consumers; T01, T02, T12 | Open |
| Production: core `model.rs`, SQLite `lib.rs`, FFI `lib.rs` | Ownership and production selection must remain explicit; read-only contexts cannot write. | All consumers; T04, T07 | Open |
| Host references: core `id.rs`, FFI binding functions | A reference includes production and object kind; external identifiers remain exact. | Manager, OTIO, Blender, Kdenlive, Natron; T12 | Open |
| Content: core `content.rs`, FFI `media_source.rs`, `representations.rs` | Legal alternatives need checked shape, members, rates, naming and roles; decoder paths must share validation. | All media pilots; T03 | Open |
| Resources/locators: core `resource.rs`, `uri.rs`, FFI `sequence_naming.rs` | Reject empty/null paths and contradictory naming without requiring current file existence. | Resolver consumers; T03, T11 | Open |
| Recognition/resolution: media source modules, FFI `resolution.rs`, `content.rs`, `known_media.rs` | Preserve ambiguity; options must report invalid setters; I/O/cancellation must stay read-only. | All media pilots; T03, T11 | Open |
| Metadata: core `metadata.rs`, registries; FFI `metadata.rs`, `metadata_input.rs` | Copy mutable inputs, enforce bounds and preserve unknown namespaces. | Manager, Blender, CLI; T03, T11 | Open |
| Provenance: core `provenance.rs`, FFI `provenance.rs` | Prevent cycles; input/output and snapshot facts must remain atomic. | Blender, Manager, OBS; T08, T11 | Open |
| Dependencies/artifacts: core and FFI `dependency.rs`, `artifact.rs` | Distinguish incomplete knowledge and closed result alternatives; bound traversal. | Manager, Blender, CLI; T03, T10 | Open |
| Jobs: core `job.rs`, FFI `jobs.rs`, executor | Job/claim pairs and caller time permit misuse; leases must bind production and validate current authority on every transition. | Manager, reference executor; T08, T09 | Open |
| Transactions: core `storage.rs`, `transaction.rs`, `conflict.rs`; SQLite `transaction.rs`; FFI `lib.rs` | Read decisions need scoped bases; commit needs its own receipt; every commit attempt is terminal. | Manager, Kdenlive, Blender, OBS; T05–T07 | Open |
| Reads/revisions: core `revision.rs`, SQLite/FFI revision modules | Separate pinned read views, head observations, wakeups, detached bases and receipts. | Kdenlive, Blender, Natron; T04, T06 | Open |
| Queries: core `query.rs`, SQLite `query_cursor.rs`, all page methods | Cursor family/filter/production/view scope and materialization bounds must be checked. | All bindings and CLI; T10 | Open |
| Errors/ownership: core `error.rs`, FFI boundary, C++ Result, Python `_errors.py` | Initialize outputs, contain panics, maintain allocator pairing and structured errors; safe owners cannot dangle. | All consumers; T07, T11, T13 | Open |
| Projections/serialization: C++ header, Python `_production.py`, `_artifact.py`, `_abi.py`, CLI modules, ABI generator | Convenience methods and decoding cannot bypass authoritative validation; all public projections need migration. | All consumers; T01–T13 | Open |
| Packaging/docs: CMake, Python manifests, workflows, `docs/examples`, site | Load the exact candidate, retain floors and installed consumption; examples must execute. | Every repository; T14–T18 | Open |

## Baseline checks

On 2026-10-04, `cargo fmt --check`, workspace/all-features Clippy with warnings
denied, workspace/all-features tests, workspace Rustdoc and
`python3 tools/check_architecture.py` passed before source edits. Local logs:
`target/api-safety-baseline/{0,1,2,3,5}.log`.
The sandbox initially prevented `cargo deny check` from locking its advisory
database. The approved unrestricted check subsequently passed; a development
rerun also passed on 2026-10-05 (duplicate-version warnings remain informational).

Supported floors remain Rust 1.85, C11, C++17 and Python 3.11. CI checks native
Linux/macOS/Windows, installed CMake consumers, C/Rust/ctypes layouts, platform
wheels, documentation examples, sanitizers and the existing Flatpak route.
Local Linux checks do not establish the other platforms or real-host runs.

## Implemented development contracts

The development series is `0.7.0-alpha.1` (Python `0.7.0a1`, ABI 47,
schema 17). ADRs 0045–0049 record pinned views, scoped edits/receipts, Python
nominal UUID hints and explicit references, production/view-scoped cursors,
and fallible C++ cancellation/options construction. Focused
regressions cover intervening writers, empty bases, terminal failed commits,
retained handles, cursor rejection and wrong-kind Python calls. Native
read sessions offer 39 native query operations. Production, asset, media-root, locator, job, activity, representation, resource, revision and
transaction IDs are distinct C/C++ values; the other native identity kinds,
validated state alternatives and
authority-controlled job leases remain open.

At `5e92680`, Linux Clippy, five storage read-session regressions, three Python
read-session regressions, typing checks and the eight coherent-example tests
passed. The example count includes four fixture-setup tests. Native C/C++
examples use an installed prefix; Python examples use source annotations and
that installed library. Earlier installed-wheel typing and downstream results
are recorded in the repository manifest. These checks do not close the audit.

At `1a7c0cb`, C++ options report setter failures immediately and retain their
previous valid state; token/options creation reports allocation failures.
Installed native contracts pass six tests, including exception-free consumption,
moved owners and recovery after invalid settings. Workspace Clippy passes.

At `734511e`, read views offer 17 native query operations, including metadata,
resolution and verification. Filesystem reads use pinned database knowledge and
current media bytes. Scoped decision tokens round-trip through Rust and the
C/C++/Python projections; editing validates their production and revision.
Seven Python view regressions and the extracted coherent recipes pass. Eight
installed native contracts pass; the library exports the expected 257 symbols.
These are development checks, not final platform or host qualification.

Subsequent job and artifact projections bring the native read-query count to
21 and the export count to 261. Nine Python view regressions pass, including
request/cancel and producing-activity changes behind retained reports.
The existing C++ artifact examples pass after shared decoder refactoring.
Full Rust workspace tests, Clippy, Rustdoc, deny and architecture checks pass
at the job-read checkpoint; artifact forwarding also passes FFI Clippy and
its installed coherent recipes. Installed wheels above predate these additions.

At `8b38d80`, dependency sets/traversal, root/unresolved/changed-object filters,
provenance/staleness, exact activity outputs and activity pages bring the native
read-query count to 34 and exports to 274. Twelve Python view regressions pass,
including dependency replacement, locator removal, journal visibility and
activity cursor family/view rejection. Source typing/lint, FFI Clippy, installed
C/C++ coherent recipes and example coverage pass. All six required Rust gates,
eight rebuilt installed native contracts, and 60 source Python tests (one skip)
pass at this checkpoint. All 64 rebuilt extracted tests pass. Doxygen,
generated C coverage and strict Sphinx pass after enabling STL support and
distinguishing the C production-ID tag from its identity-read function.
C/Rust/ctypes layouts agree. Final candidate qualification remains open.

The journal follow-up at `95e2f51` adds retained heads, bounded revision pages
and filtered pages: 37 native read queries and 277 exports. SQLite reuses an
existing pinned transaction for filtered reads; live reads retain their own
short snapshot. Five storage view tests, five live revision tests, thirteen
Python view regressions and all three installed coherent recipes pass.
Storage/FFI Clippy, source Ruff/ty, example coverage and direct C failure-output
checks pass. The journal follow-up also passes full workspace tests, 61 source
Python tests (one skip), Doxygen, generated C coverage and strict Sphinx.

At `9905df7`, bounded revision-event pages and regeneration planning bring the
native read-query count to 39 and exports to 281. ADR 0050 records event bounds
and cursor scope. Six storage view regressions and fourteen Python view
regressions pass, including ordered continuation, future-revision rejection,
retained planning parameters and bounded iterable consumption. Workspace
Clippy, source Ruff/ty, all three installed coherent recipes, direct C output
checks, example coverage and export comparison pass. The full source Python
suite runs 62 tests with one skip. Subsequent ABI 40 checks below include these
operations; final qualification remains open.

At `7635cf5`, native revision and transaction IDs complete this identity slice.
ABI 40 takes revision arguments by value and types journal/receipt outputs;
285 exports and all 18 C/Rust/ctypes layouts agree. All six required Rust gates,
eight installed native contracts and 64 extracted tests pass. Source and
installed-wheel Python suites each run 63 tests with one skip; Ruff/ty pass.
Doxygen, generated C coverage and strict Sphinx pass after `8965f03` hides
implementation-only hash specializations from Breathe. These checks preserve
the released baseline and do not close the remaining audit rows.

At `a56c0f1`, C has checked job claim/completion/failure accessors; C++ and
Python have one status alternative instead of independent optional payloads
(ADR 0051). Wrong-state, missing-index, null/output-clearing and malformed
projection regressions pass. ABI 40 now has 288 exports and 20 agreeing public
struct layouts. Workspace tests, Clippy, Rustdoc, fmt and architecture checks
pass; dependency inputs are unchanged from the successful deny check above.
Source and installed-wheel Python each run 66 tests with one skip; Ruff/ty,
eight rebuilt native contracts and strict docs pass. The 64 extracted tests
pass after the C++/Python changes; the updated C job recipe then passes
separately. The flattened C job view and claim secrecy/time remain open.

At `98e1cc5`, every Python transaction context requires explicit commit;
uncommitted normal/exceptional exits roll back. Explicit close and failed
commit never trigger another mutation on exit. SDK callers and six Python
consumers have migrated. Source and installed-wheel suites run 68 tests with
one skip; Ruff/ty, all 16 extracted Python tests and strict Sphinx pass.
The installed Python quickstart passes. Unbased mutation restrictions and
remaining CLI receipt paths are still open.

Repository inputs and migration status are in
[`api-safety-repositories.md`](api-safety-repositories.md).

## Typed asset operations checkpoint

SDK runtime `26c2ec6` (0.7.0-alpha.1 / Python 0.7.0a1 / ABI 41 / schema 17)
carries asset IDs through every asset argument and asset-bearing result field.
Small C scalar inputs are values; arrays borrow typed IDs and outputs write
stack values. Checked reference construction/projection avoids caller retagging.
C++ imports return `AssetId`; Python runtime values remain ordinary UUIDs.
Other native IDs and the final dynamic-reference model remain open.

All six required Rust gates pass. Eight installed native contracts, all 64
extracted examples, Doxygen coverage and strict Sphinx pass. The 292 exported
symbols match the manifest; all 21 Rust/C/ctypes layouts agree. Positive and
negative installed identity fixtures pass. Tests reject nil, wrong-kind and
wrong-production asset reads without treating parsing as existence.
The repository manifest records exact wheel and host-check scope. These
checks do not close the audit or qualify a release candidate.

## Metadata decision checkpoint

SDK source `a73598b` retains ABI 41/schema 17, 292 exports and 21 layouts.
ADRs 0045/0052 require a base for metadata replacement/removal. Appends merge
while advancing the property version checked by destructive edits. Rust,
direct C, C++/Python and CLI regressions cover stale decisions, atomic rollback
and usable transactions after early rejection. CLI metadata JSON returns atomic
receipts; structured conflicts now go to stdout.

Native library/header inputs are `84650bf`; Python inputs are `191c8c4`.
The subsequent source-package manifest commit changes no runtime input.
All six required Rust gates, eight fresh installed native contracts, all 64
extracted tests and strict Sphinx pass. Source and installed-wheel Python each
run 73 tests with one skip; Ruff, ty and installed identity fixtures pass.
Binary metadata is bounded before copying and resolution arrays are checked
for addressability before access. Asset existence uses a point query, propagates
requested-row corruption and avoids materializing unrelated rows.

Logs use `target/api-safety-metadata-*`; the repository manifest records the
matching artifacts and consumer scope. Other unbased mutation families, native
ID kinds, leases, bounded materializers and final qualification remain open.
No audit row or completion gate is closed by this checkpoint.

## Typed media-root checkpoint

SDK `50a6027` retains schema 17 and uses ABI 42: 294 exports, 22 agreeing
public layouts and 39 native read queries. Root creation/mutation, summaries,
revision events and conflict outputs carry a distinct native identity. C scalar
mutations take values. C++ conflict targets are `ObjectRef`/`MediaRootId`
alternatives; Python keeps ordinary UUIDs. Direct C/C++/ctypes checks reject
wrong native kinds. Runtime tests reject nil/foreign roots without a revision.

All six required Rust gates, eight installed native contracts, all 64 extracted
tests, strict docs and source Ruff/ty pass. Source and installed-wheel Python
each run 75 tests with one skip. Installed typing checks reject five wrong-kind
calls. The platform-wheel quickstart passes without library overrides. Matching
consumer/artifact scope is in the repository manifest; logs use
`target/api-safety-root-*` (Kdenlive uses `api-safety-kdenlive-root-*`).
Other identity/value families, write restrictions, leases, bounded-query audit
and final host/platform/handoff qualification remain open.

## Root decision checks

At `a6e3907`, enabling/removing roots requires a decision base before staging,
including idempotent requests. Creation remains additive. Storage, C and Python
reject invalid inputs without poisoning an open edit; stale removal reports
the typed root conflict. CLI commands require inspection tokens and return
their own receipt or structured conflict. Recipes use read-bound edits.

On 2026-10-06, all six Rust gates, eight installed native contracts, 64 extracted
tests, strict docs, symbol/layout checks and source Ruff/ty pass. Source and
installed-wheel Python run 77 tests with one skip; installed typing rejects
five wrong-kind calls. ABI 42/schema 17 retain 294 exports and 22 layouts.
Logs and installed files use `target/api-safety-root-edits-*`. Consumer evidence
remains the preceding typed-root checkpoint; no completion gate is closed.

## Typed locator checks

SDK `2501860` uses ABI 43/schema 17: 296 exports, 23 agreeing public layouts
and 39 native read queries. Locator retirement takes a value; locator reads
and revision events return typed identities. C++ preserves equality, ordering,
hashing and explicit interchange. Python retains ordinary UUIDs and validates
value shape, existence and scope through the native store.

On 2026-10-06, all six Rust gates, eight installed native contracts, 64 extracted
tests, strict docs and symbol/layout checks pass. Source and installed-wheel
Python run 79 tests with one skip; six installed wrong-kind calls are rejected.
Matching consumer builds and artifact hashes are in the repository manifest.
Logs use `target/api-safety-locator-*`; Kdenlive uses `api-safety-kdenlive-locator-*`.
Remaining identities, write restrictions, leases and final qualification stay open.

## Typed job checks

At `28e24d9`, ABI 44/schema 17 has 300 exports, 24 agreeing layouts and
39 native read queries. Job operations, summaries, regeneration plans and
revision events carry distinct native IDs. Checked C/C++ reference helpers
support job metadata targets. Python retains UUID values, rejects malformed
input before conversion and checks nil/foreign job identities through storage.
Claim IDs, clock inputs and lease authority remain separate unfinished work.

On 2026-10-06, all six Rust gates, eight installed native contracts, 64 extracted
tests, strict docs and symbol/layout checks pass. Source and installed-wheel
Python run 81 tests with one skip; eight installed wrong-kind calls fail typing.
Kdenlive's updated job fields replay in all ten patches and compile with host
flags. Matching consumer and artifact scope is in the repository manifest;
logs use `target/api-safety-job-*` and `api-safety-kdenlive-job-*`.
No audit row or completion gate is closed.

## Typed activity checkpoint

SDK `eedfc35` carries activity IDs through creation, provenance, artifact
explanations, job completion and revision events. ABI 45/schema 17 has 304
exports and 25 agreeing C/Rust/ctypes layouts. C scalar completion arguments
are values; C++ creation returns `ActivityId`. Checked dynamic references
reject wrong kinds. Python retains standard UUIDs and validates target scope.

All six Rust gates, eight installed native contracts, 64 extracted tests,
strict docs and source lint/typing pass. Source and installed-wheel Python
run 83 tests with one skip; nine wrong-kind typing calls reject. The repository
manifest records matching wheel/bundle and consumer evidence. Other native
identity kinds, state alternatives and leases remain open.

## Typed representation checkpoint

SDK `ec7c38a` carries representation IDs through native operations, reads,
resolution, dependency paths, artifact reports, activity edges, jobs and
revision events. ABI 46/schema 17 has 308 exports and 26 agreeing layouts.
Required scalar C inputs use values; optional filters and arrays borrow typed
IDs. C++ references project checked representation identities from saved
bindings. Python retains standard UUID values.

All six Rust gates, eight installed native contracts, 64 extracted tests and
strict docs pass. Source and installed Python run 85 tests with one skip;
source lint/typing and nine installed wrong-kind calls pass. Matching artifacts
and scoped consumer results are in the repository manifest. Resources,
leases, remaining state alternatives and final qualification remain open.

## Typed resource checkpoint

SDK `1ce127e` carries resource IDs through membership, locators, verification,
resolution, dependency paths and revision events. ABI 47/schema 17 has 312
exports and 27 agreeing layouts. Required C scalars use values; C++ resource
pages preserve `ResourceId`. Python keeps standard UUID values. Checked dynamic
reference projections replace generic-byte calls in the executable recipes.

Six Rust gates, eight installed native contracts, 64 extracted tests, strict
docs, source lint/typing and 20 tooling tests pass. Source/installed Python:
87 run, one skip; nine installed wrong-kind calls reject. Matching artifacts
and consumer scopes are recorded in the repository manifest. Dynamic-reference
construction, remaining state alternatives, write restrictions, leases and
final qualification remain open.

## CLI receipt checkpoint

SDK `bc74828` reports own receipts for explicit media, representation,
locator/confirmation, identifier, dependency, activity and job writes, alongside
metadata and root receipts. No-change observations return no revision. Tests
cover later writes, duplicate failure and ordered job transitions. CLI tests,
Clippy, all 16 installed CLI recipe tests and strict docs pass. The installed
CLI test prefix is `target/api-safety-cli-receipts-install` (debug CLI, matching
ABI 47 native library); this is not a release artifact. Executor outcome/error
receipts and the remaining write restrictions are still under review.
