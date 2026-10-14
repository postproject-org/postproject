# PostProject 0.9 / ABI 57 qualification

Prepared `0.9.0-alpha.1`, Python `0.9.0a1`, C ABI **57 / 344 exports /
45 layouts**, schema **27**, exchange framing **1** and CLI JSON **1**.
This revised candidate is unpublished. The original
{doc}`ABI-53 report <release-0.9-report>` and nineteen-artifact set retain
their historical scope. All required integrated checks pass for the source and scope below.

## Delivered workflows

Import returns both asset and original-representation identities. Path-based
lookup removes conversion wrappers without merging assets. Verified relink
selection retains copied evidence and its original coherent base; confirmation
stages all locations together, with guarded no-op receipts for known locations.
Output registration stages media, actual activity, input roles, parameters and
evidence atomically. Real jobs compose guarded completion in that same edit.

Native edits accept attribution at creation. C++ has copied target-wide metadata
reads and both Result/exception styles. Python reads and writes unambiguous
metadata as scalars and immutable tuples; explicit tags preserve ambiguous
meanings and ordered repeated fields. Worker and exchange values have explicit
modules. The migration map removes superseded forms across all five surfaces.
ADRs 0076–0084 record these decisions; no new standards mapping is introduced.

Provenance inputs do not invent authored dependencies. A separate scene example
records actual references. Knowledge state, availability and work state retain
their separate meanings; recorded recipe completeness does not establish
executable or deterministic reproduction.

## Local evidence

| Boundary | Completed check |
|---|---|
| Rust | Six required gates; exact Rust 1.85 tests and doctests |
| Native | Nine installed contracts, 344 exports and 45 Rust/C/ctypes layouts; both C++ styles |
| Memory/ownership | Nine ASan/UBSan contracts; relocated installed consumers with Cargo prohibited |
| Python | Final licensed wheel: 157 tests on both 3.11 and 3.14, installed CLI enabled; fifteen wrong-ID negatives and positive typing |
| Examples | 96 extracted installed checks; strict generated documentation and coverage |
| Packages | Installed Manager 0.6.0 and demo 0.4.0; Manager nine tests on both supported OpenAssetIO dependency sets |
| Real hosts | Blender 5.2.2: 24 passed/one future-host skip; all eight OBS 32.2.2 scenarios; Natron 2.5.0 normal/negative/plugin-free renderer |
| Native pilots | Ardour 9.8 installed resolver; complete Kdenlive 26.08.2 build and two test suites |
| Shared production | Both OBS → Blender and Kdenlive → Blender → Manager pass identity, relocation, output, conflict and staleness checks |

The {doc}`five-task comparison <release-0.9-workflow-comparison>` preserves
runnable qualified-0.9 before/current after programs. The
{doc}`measurements <release-0.9-ergonomics-measurements>` retain schema-27
baselines, new samples, hashes and explicit allocation/timing limits.

Strict selection rejects sampled-only evidence. The hosts retain explicit
policies: Kdenlive also checks its clip hash, Blender and Ardour can reuse an already
recorded sampled file address, and sequence pilots require complete frames
before accepting sampled collection evidence. These policies do not establish
full-content verification. No helper guarantees filesystem atomicity.

## Source and CI scope

The {download}`consumer source record <evidence/release-0.9/abi57-consumers.json>`
names all thirteen maintained repositories and scoped checks. Runtime payloads
come from SDK `9bb89993`; subsequent SDK changes record evidence, format tools,
initialize a C example receipt, strengthen version checking and improve test
assertion diagnostics. They do not change native or Python runtime bytes.

| Pipeline and source | Individual conclusions |
|---|---|
| [SDK 38628074732](https://github.com/postproject-org/postproject/actions/runs/38628074732), `4c6c0563` | All 16 jobs pass, including Linux/macOS/Windows, installed packages, glibc-2.28 wheel, MSRV, sanitizers and strict docs/links |
| [Downstream 38628076081](https://github.com/postproject-org/postproject/actions/runs/38628076081), `4c6c0563` | All 11 jobs pass, including full Ardour with/without SDK and both Manager dependency sets |
| [Kdenlive 38628074380](https://github.com/postproject-org/postproject/actions/runs/38628074380), `d5f6a81` | Full with/without-SDK builds, tests and handoff pass |
| [Blender 38628064357](https://github.com/postproject-org/postproject/actions/runs/38628064357), `35f77c0` | Stable 5.2.2 and lint pass; optional 5.3 beta fails |

The {download}`individual job record <evidence/release-0.9/abi57-ci.json>`
retains exact inputs and conclusions. Earlier formatting, current-Clippy and
MSVC example failures were corrected; failed runs remain in the record.
Final report/checksum edits reuse these checks with an explicit source comparison;
strict documentation and deployment are checked for the final records.

## Distribution and limits

The prepared set contains native shared/static libraries, CLI, headers,
relocatable CMake/pkg-config metadata, typed platform wheel, Blender extension,
Manager/demo wheels and thirteen source archives. Native packages include
dependency notices. These local Linux artifacts are **workstation builds**;
the separate glibc-2.28 CI wheel has its own platform qualification.
The {download}`artifact record <evidence/release-0.9/abi57-artifacts.json>`
identifies source snapshots, payload hashes and all nineteen prepared files.
Its manifest and checksums remain outside the archives to avoid self-reference.

Deferred location confirmation uses `locator.confirm.v1`; older experimental
decoders reject that command explicitly. Schema, existing record encodings and
protocol framing are unchanged. Bindings and native library must match ABI 57.

The pinned OTIO linker retains its existing deprecation warning. Optional
Blender 5.3 beta `346d005c9994` fails project-variable relocation and completed
render fixtures. Licensed Nuke execution and a second independent real worker
remain unexecuted. Stable host support does not imply other-host compatibility,
timeline conversion, media transfer, editable replicas or upstream endorsement.

Docs/site deployment is authorized and recorded separately from previews.
The unchanged `f823e7f` landing scenarios preserve the existing theme. Tags,
registry publication and outreach have not been authorized or performed.
