# PostProject 0.8 qualification

Package `0.8.0-alpha.1` (Python `0.8.0a1`), C ABI **52**, **349 exports**,
**36 public layouts**, SQLite schema **27**, experimental protocol major **1**.
The qualification baseline is `v0.7.0-alpha.1`, commit
`e14d92fbcd7de55626b7da3a42aee4e8d3a32ba6`.
Qualification completed on 2026-10-11 against the source and artifacts below.

## Delivered behavior

One local authority accepts ordered atomic proposals and retains original
changing, no-op and terminal rejected outcomes. Existing native domain writes
capture complete committed records automatically. A checkpoint and contiguous
suffix reconstruct passive stores through ordinary read APIs, preserving exact
identity, compound content, metadata order, locators, fingerprint history,
dependency/activity evidence and inert job observations.

Rust, C11, C++17, Python and CLI expose preparation/submission, original-outcome
recovery, scoped inspection, checkpoint exchange and catch-up. Native callers
use typed staging operations; Python also offers ordered command collections.
Worker ownership is delivered separately from public outcomes and never reissued
by duplicate lookup. Mirrors reject ordinary edits and worker execution.

ADRs 0065–0067 and the {doc}`exchange-protocol` define encoding, bounds, history,
credential delivery and failure contracts. The
{doc}`src/integrators/portable-exchange` provides installed caller recipes.
No network service, editable replica, authority promotion, media transfer,
automatic synchronization or new standards mapping is delivered.

## Verification and artifacts

The six required local Rust gates pass, including workspace all-feature tests
and doctests. The dependency check passed after retrying advisory-cache access
outside the read-only sandbox. Installed checks pass: nine native contracts,
82 guide examples, 349 symbols, Rust/C/ctypes layouts, Python 3.11/3.14 tests
(132 passed, one skipped each), and positive typing plus twelve expected
wrong-kind failures on each interpreter. Strict Doxygen/Sphinx, inventory,
architecture and example coverage pass.

The {doc}`release-0.8-conformance` records independent wire/digest fixtures,
process termination and restart, submission races, atomic rejection and complete
two-mirror convergence through a real Blender native history. Local bounded
ASan fuzzing and measured workloads retain their development source provenance;
they are not relabelled as new package-version runs.

{download}`Installed candidate provenance <evidence/release-0.8/linux-candidate.json>`
records native/header/wheel checksums and their source commits. Native
implementation is `016297a`; installed header qualification is `a541752`;
wheel binding source is `8d05e6f`. The final local CLI additionally refreshes its
help description at `6c7ee03`, with its installed exchange recipe repeated.
{download}`Review artifact inventory <evidence/release-0.8/review-artifacts.json>`
records source/Linux/macOS/Windows archives and wheels with checksums. The Linux
archive's relocated C/C++ and installed-wheel Python quickstarts pass without
Cargo. macOS/Windows review artifacts reuse qualified CI install prefixes and
libraries; they were not executed on this workstation. These are local candidate
artifacts, not published assets. The local Linux build makes no glibc 2.28 claim.

All sixteen [SDK CI jobs](https://github.com/postproject-org/postproject/actions/runs/38149795513)
pass at `dfad96b`, including exact Rust 1.85, Linux/macOS/Windows tests and
installed packages, ASan/UBSan consumers, real FFmpeg, docs/links/spelling and
the separate glibc 2.28 wheel. Both
[documentation jobs](https://github.com/postproject-org/postproject/actions/runs/38149795304)
and the [KDE 6.10 Flatpak check](https://github.com/postproject-org/postproject/actions/runs/38146446748)
pass. {download}`Individual SDK conclusions <evidence/release-0.8/sdk-jobs.json>`
retain optional failures and distinguish unfinished jobs from passes.
All eleven [downstream jobs](https://github.com/postproject-org/postproject/actions/runs/38149795827)
pass at `dfad96b`, including full Ardour builds with and
without PostProject. The corrected
[Kdenlive repository workflow](https://github.com/postproject-org/postproject-kdenlive/actions/runs/38150060649)
and [SDK full-host workflow](https://github.com/postproject-org/postproject/actions/runs/38150158593)
pass with/without PostProject and repeat the shared handoff.
{download}`Final completed jobs <evidence/release-0.8/final-jobs.json>`
record the exact source revisions and individual conclusions.

## Migration, consumers and compatibility

Supported older-schema fixtures migrate without fabricated replay history.
The installed candidate also opens a production authored by the released 0.7
CLI, preserving identity, revision/events and exact unknown metadata while
establishing a replay floor at the retained head. That released CLI then returns
`unsupported` for schema 27 and leaves the file byte-for-byte unchanged.
{download}`Migration evidence <evidence/release-0.8/schema-migration.json>`
pins both binaries and verifies baseline source. Older SDKs cannot write mirrors.
Explicit development-history resynchronization preserves earlier evidence and
outcomes while establishing a new checkpoint floor; it does not restore authority.

[The repository manifest](exchange-repositories.json) accounts for all thirteen
active repositories with exact revisions, version bounds, host pins and commands.
Both real handoffs pass against the installed candidate: OBS 32.2.2 → Blender
5.2.2, and Kdenlive 26.08.1 → Blender → Manager 0.5.0, including moves, provenance,
competing decisions, refresh and staleness. Natron 2.5.0 passes normal, negative
and plugin-free renderer paths. Ardour's installed audio resolver passes.
These are automated host paths; no interactive acceptance, full Natron rebuild
or upstream endorsement is claimed.

Python consumers and native contract jobs pass.
{download}`Individual consumer conclusions <evidence/release-0.8/consumer-jobs.json>`
retain their scope. Blender's supported 5.2.2 and lint jobs pass; its optional
5.3-alpha job fails at download because no daily Linux build is available.

The {doc}`release-0.8-compatibility-evidence` reviews released 0.7 → candidate
family changes and dependency closure using actual installed host traces.
No additional stable family or cross-series binary replacement is promised.
The 0.6.x C++ Result propagation promise remains intact. A second independent
real consumer of renew/fail/cancel remains absent; synthetic worker conformance
does not close that adoption gap.

The {doc}`schema-27-exchange-measurements` reports measured capture costs:
10,000-asset creation increased 23%, median individual import transactions 17%,
and retained record storage explains almost all measured disk growth. Streaming,
maximum binary/nesting and outcome retention are measured explicitly. These
shared-workstation samples establish no universal latency or memory threshold.

## Commercial and publication scope

The [Nuke 17.0v1 Read package](https://github.com/postproject-org/postproject-openassetio-manager/blob/ccfbd8d58a7b2da7b607940917aeea30cd697de0/docs/nuke-trial.md)
includes configuration, real PNG sequence fixture, acceptance script and
plugin-free copy instructions. License-independent checks pass with both the
bundled and current OpenAssetIO/MediaCreation combinations. Licensed execution
is **not executed**: no Nuke binary/license is available here. The brief supplies
the exact ready-to-run command. {doc}`commercial-host-feasibility` records the
reviewed REAPER and vendor-shipped Resolve routes without runtime claims.

Qualification did not include release tagging, registry uploads or consumer
package publication. Published SDK artifacts are listed in
[GitHub releases](https://github.com/postproject-org/postproject/releases).
The publication procedure is documented in {doc}`releasing`.
