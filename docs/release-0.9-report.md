# PostProject 0.9 development qualification

Prepared `0.9.0-alpha.1` (Python `0.9.0a1`), C ABI **53**, **344 exports**,
**38 public layouts**, SQLite schema **27**, exchange protocol **1** and CLI
JSON **1**. This is an unpublished development candidate, checked locally on
2026-10-12 against published `v0.8.0-alpha.1` (`7e097bd`). Required platform
and full Ardour CI remain unexecuted; this report does not declare acceptance.

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
| Python | Installed 3.11 and 3.14: 143 passed, one skipped each; positive typing and twelve wrong-ID negatives |
| Examples/docs | 84 installed extracted examples; Doxygen coverage; strict Sphinx, spelling and offline links |
| Consumers | Manager: eight tests; each OpenAssetIO/OTIO demonstration and sample host; native C++ NLE, OBS and Natron contracts |
| Real hosts | Blender 5.2.2: 24 passed, one future-version skip; OBS 32.2.2 normal/error/disabled paths; Natron 2.5.0 normal/negative/fallback |
| Handoffs | OBS → Blender; Kdenlive 26.08.2 → Blender → Manager, including identity, moves, conflicts and stale outputs |
| Exchange | Actual Blender-authored history reconstructed in two independent passive processes, including duplicate apply and reopen |
| Clean packages | Relocated native archive: nine contracts with Cargo prohibited; fresh installed Manager/demo wheels and bundled SDK loading |

The pinned OTIO upstream linker emits its known unversioned-trait deprecation
warning; that warning is retained. The actual host exchange fixture has three
assets, two activities and no jobs; it is not independent worker evidence.
Ardour's installed native audio resolver passes, but its full local configure
is blocked by the missing mandatory `liblo` dependency.

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
- **Deployment:** documentation and desktop/mobile light/dark website previews
  pass. Main-branch pushes and their automatic Pages deployments await approval.
- **Integration guides:** Blender, Kdenlive and Nuke have setup, validation and
  support notes. No upstream acceptance, endorsement or official listing is claimed.

No second independent worker executes renewal, failure or cancellation here.
Automated host fixtures do not establish a human interactive or commercial-host
pass. No network service, media transfer, implicit synchronization or editable
replica is delivered.
