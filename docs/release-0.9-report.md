# PostProject 0.9 development qualification

This report records the qualified `f456958c` candidate. Additional workflow
changes are being implemented under the {doc}`api-workflow-0.9-design`; their
qualification is separate. The earlier package/checksum set remains intact.

Prepared `0.9.0-alpha.1` (Python `0.9.0a1`), C ABI **53**, **344 exports**,
**38 public layouts**, SQLite schema **27**, exchange protocol **1** and CLI
JSON **1**. This unpublished development candidate qualifies against published
`v0.8.0-alpha.1` (`7e097bd`) on 2026-10-12. Required local, platform and
maintained-integration checks pass; external limitations remain below.

## Delivered behavior

Python uses one `Edit` type for additive, retained-view and detached decisions,
with consistent context keywords and explicit commit. Ordinary metadata keys
and scalar values use checked native inputs; typed values retain all lossless
domain meanings. Single files stage directly from paths. Python and Rust offer
bounded lazy scans owned by the caller's read view. C/C++ resolution options
are constructed atomically, and Rust/C/C++ commit returns the caller's receipt
through one operation. The CLI has one scoped decision option.

ADRs 0068–0075 record these source/API changes. Transaction terminality,
conflicts, lease fencing, complete native capture, passive replay, copied-value
ownership and structured errors retain their meanings. No storage or protocol
change, new standards mapping or stable API family is introduced. Historical
0.8 artifacts are unchanged.

The {doc}`src/integrators/api-migration-0.9` provides five runnable Python
before/after tasks and corresponding native, Rust and CLI recipes.
The {doc}`api-ergonomics-audit` records caller costs and design choices; the
{doc}`release-0.9-ergonomics-measurements` records installed 0.8/0.9 workloads
and their measurement limits.

## Completed local checks

| Boundary | Evidence |
|---|---|
| Rust | All six required gates; exact Rust 1.85 workspace checks and tests |
| Native SDK | Nine installed contracts; 344 symbols; 38 Rust/C/ctypes layouts; both C++ error styles |
| Memory checks | Nine ASan/UBSan native contracts and sanitized C smoke |
| Python | Installed 3.11 and 3.14: all 143 tests pass with the installed CLI enabled; positive typing and twelve wrong-ID negatives |
| Examples/docs | 84 installed extracted examples; Doxygen coverage; strict Sphinx, spelling and offline links |
| Consumers | Manager: eight tests; each OpenAssetIO/OTIO demonstration and sample host; native C++ NLE, OBS and Natron contracts |
| Real hosts | Blender 5.2.2: 24 passed, one future-version skip; OBS 32.2.2 normal/error/disabled paths; Natron 2.5.0 normal/negative/fallback |
| Handoffs | OBS → Blender; Kdenlive 26.08.2 → Blender → Manager, including identity, moves, conflicts and stale outputs |
| Exchange | Actual Blender-authored history reconstructed in two independent passive processes, including duplicate apply and reopen |
| Clean packages | Relocated native archive: nine contracts with Cargo prohibited; fresh installed Manager/demo wheels and bundled SDK loading |

The pinned OTIO upstream linker emits its known unversioned-trait deprecation
warning; that warning is retained. The actual host exchange fixture has three
assets, two activities and no jobs; it is not independent worker evidence.
Ardour's installed native audio resolver passes. Its full local configure
remains blocked by the missing `liblo` dependency; full CI results follow.

## Completed CI

| Pipeline | Verified result |
|---|---|
| [SDK](https://github.com/postproject-org/postproject/actions/runs/38229673079), `3d1b5f51` | All 16 jobs pass, including Linux/macOS/Windows tests and installed native packages, glibc-2.28 wheel, MSRV and sanitizers |
| [Downstream](https://github.com/postproject-org/postproject/actions/runs/38229673775), `3d1b5f51` | All 11 jobs pass, including both full Ardour builds and both OpenAssetIO dependency sets |
| [Kdenlive](https://github.com/postproject-org/postproject-kdenlive/actions/runs/38228465707), `09bd9bf` | Full builds and tests with and without PostProject pass |
| [Blender](https://github.com/postproject-org/postproject-blender/actions/runs/38230174201), `ef5274a` | Stable 5.2.2, optional 5.3 beta and lint all pass |

The {download}`CI source and job record <evidence/release-0.9/ci-qualification.json>`
identifies the checked revisions. Downloaded Linux, macOS and Windows native
packages have matching public headers, allowing Windows line endings. The Linux
package's 344 exports and installed Python quickstart pass locally. The earlier stale inventory, helper
assertion and Rust 1.99 lint failures were corrected; these results supersede
those failed runs.

## Prepared artifacts and application targets

The review set includes headers, shared/static libraries, CLI, relocatable
CMake/pkg-config metadata, typed Python wheel, Blender extension, Manager/demo
wheels and source archives for all 13 repositories. Native packages and wheels
include SDK licenses and an inventory of 91 Linux native dependency notices.
They are workstation Linux artifacts, **not qualified manylinux releases**.
The external artifact manifest and checksums identify exact inputs separately
from their archives; source and repository provenance are recorded in the
[repository inventory](api-ergonomics-repositories.json).

Kdenlive 26.08.2 (`ef543982`) is the new pinned stable target. Its ten patches
apply after adapting the proxy wait hooks to its member-owned process. Full
with/without-SDK builds, two enabled/one disabled test suites and the complete
shared-production handoff pass.
Blender 5.2.2 LTS, OBS 32.2.2, Natron 2.5.0 and Ardour 9.8 retain their verified
stable pins. Historical host qualification remains release-scoped.

## External outcomes and limits

- **Licensed Nuke:** not executed. Manager/configuration/sequence tests pass
  against both standalone and Nuke 17 shipped OpenAssetIO dependency pins.
- **Public releases:** prepared and unpublished; no tags or registry uploads.
- **Deployment:** the practical pilot descriptions at [postproject.org](https://www.postproject.org/)
  match site revision `59a5f25`; both deployment jobs pass. Documentation
  source `3d1b5f51` is deployed and checked live; strict local documentation
  and six website previews pass.
- **Integration guides:** Blender, Kdenlive and Nuke have setup, validation and
  support notes. No upstream acceptance, endorsement or official listing is claimed.

No second independent worker executes renewal, failure or cancellation here.
Automated host fixtures do not establish a human interactive or commercial-host
pass. No network service, media transfer, implicit synchronization or editable
replica is delivered.
