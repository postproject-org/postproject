# API safety repository inputs

Inventory checked against `gh repo list postproject-org` on 2026-10-04.
All 13 non-archived organization repositories have local checkouts; the
12-repository release evidence excludes the landing site. All start on `main`.
Only untracked build directories were present. Final candidate migration and
host/platform qualification remain pending for every row. The development
checks below verify specific changes, not the coordinated release.

| Repository (workspace path) | Baseline SHA | Role / consumed surface | Status |
|---|---|---|---|
| [postproject](https://github.com/postproject-org/postproject) | `a413227b9dd048f744fa7393d8927891ee93d5e2` | SDK / Rust, C, C++, Python, CLI | Pending |
| [postproject-ardour](https://github.com/postproject-org/postproject-ardour) | `7ec34c2a972b936191494f25d420c4e962f8c047` | Audio resolver / C++ / pkg-config | Pending |
| [postproject-blender](https://github.com/postproject-org/postproject-blender) | `8c62d4ca6243446219ce82383dd3ed823f850c3c` | Extension / Python / pinned Blender | Pending |
| [postproject-cpp-nle](https://github.com/postproject-org/postproject-cpp-nle) | `5d801c0b59526a098b9111beeb40b143bbe59ca1` | Installed C++ test host | Pending |
| [postproject-kdenlive](https://github.com/postproject-org/postproject-kdenlive) | `2eb45f9019cdb48fdc35606768c9ecc164943b8f` | Native pilot / C++ Result / Flatpak | Pending |
| [postproject-natron](https://github.com/postproject-org/postproject-natron) | `4694e97a65ab59954a3ef32c7984ed6ccd53a97f` | Sequence Reader / C++ / CPython stable ABI | Pending |
| [postproject-obs](https://github.com/postproject-org/postproject-obs) | `5c15a4a7fc39d4e22d191c3fc854915f098931c9` | Recording adapter / C11 / Qt frontend | Pending |
| [postproject-openassetio](https://github.com/postproject-org/postproject-openassetio) | `99ab45e38dc31b2ff5f8e63cd9a848f090cc389d` | OpenAssetIO test host / Python | Pending |
| [postproject-openassetio-manager](https://github.com/postproject-org/postproject-openassetio-manager) | `796631907817695cf33ca6a0276bb243bc54a099` | Maintained Manager / Python / jobs | Pending |
| [postproject-otio-demo](https://github.com/postproject-org/postproject-otio-demo) | `bb67d3483acb0244906d35692bd1304ff2966a80` | Manager + OTIO demonstration / Python | Pending |
| [postproject-otio-openassetio](https://github.com/postproject-org/postproject-otio-openassetio) | `afcd6503f5286b0ce3fa2ae4165340f8ef86b690` | OTIO/OpenAssetIO test host / Python | Pending |
| [postproject-python-host](https://github.com/postproject-org/postproject-python-host) | `98e7ee184f6a795bd42c99d331724157fd6235dc` | Installed Python test host | Pending |
| [postproject.org](https://github.com/postproject-org/postproject.org) | `606f228b61adefe1748b029b72228ec549dd37cb` | Landing site / links and version pins | Pending |

The local `postproject-shotcut` repository has no remote and contains the
previously rejected pilot scaffold. It is historical, not a newly selected
integration. No Audacity, Krita or GES/Pitivi integration was found.

All checkouts inherit the workspace `AGENTS.md`; no nested `AGENTS.md` was found.
No new GitHub repository is needed for these local API changes. Publication of
a release tag requires separate explicit authorization.

Each migrated row must record its consumer SHA, exact installed SDK build/pin,
host/toolchain version, commands or CI links, ordinary/failure/removal results
and platform limits before it can be marked verified. Historical 0.6 passes do
not qualify as candidate evidence.

## Python reference migration checkpoint

These consumer commits replace runtime ID-class tests and bare dynamic targets
with explicit SDK references. Tests used an installed neutral wheel containing
the development UUID hints and typing marker, plus an installed Linux native
library. This historical checkpoint used SDK `9d5afc8` (ABI 38, schema 17)
with the temporary package version `0.6.0a1`. Wheel SHA-256:
`bd1f5d787305fdbb17631337f4a94f3184c8a4fe76b11f0d43dee7b6e0b245d8`.

| Consumer commit | Executed result | Scope |
|---|---|---|
| Manager `c960d49` | pytest: 6 passed | Read resolution and publishing contract tests |
| OpenAssetIO validation `a7f6f3b` | pytest: 1 passed | Installed SDK resolution |
| OTIO validation `e85e0b5` | pytest: 1 passed | Upstream linker round trip |
| Python host `bc7e568` | pytest: 1 passed | Import, persisted reference, resolve |
| OTIO demo `aad9f5f` | pytest: 1 passed | Maintained Manager/linker round trip |
| Blender `7724f40` | 19 passed, 1 skipped | Blender 5.2.2 LTS background suite; project-variable case requires 5.3 |

The OpenAssetIO/OTIO checks used Python 3.13, editable consumer installs and the
linker pin `ee762d7d24670f84c30baa6149123b6d252a9969`. Reproduce from each consumer:

```sh
uv run --no-project --python 3.13 --with-editable . --with pytest \
  --with /path/to/postproject.whl \
  --with-editable ../postproject-openassetio \
  --with-editable ../postproject-openassetio-manager \
  --with git+https://github.com/OpenAssetIO/otio-openassetio.git@ee762d7d24670f84c30baa6149123b6d252a9969 \
  env POSTPROJECT_LIBRARY=/path/to/installed/libpostproject.so python -m pytest -q
```

Blender used `tools/build.py` with a platform wheel containing that SDK, then
`tools/test.sh blender/blender /path/to/postproject_relink-0.1.0-linux_x64.zip`.
No interactive or cross-host handoff run is claimed.

## 0.7 development checkpoint

At this checkpoint, the ten edited repositories' temporary branches were
fast-forwarded into local `main` and deleted. Origin had not changed.
The SDK is `1a7c0cb`, package `0.7.0-alpha.1` / Python `0.7.0a1`, ABI 38,
schema 17. Python consumers require `postproject>=0.7.0a1,<0.8`; the Manager
is 0.4.0 and the OTIO demo is 0.2.0 with `postproject-openassetio-manager>=0.4,<0.5`.

| Consumer commit | Executed result against 0.7 | Scope |
|---|---|---|
| Manager `662de75` | 6 passed | Installed-wheel contract tests |
| OpenAssetIO validation `a362568` | 1 passed | Installed-wheel resolution |
| OTIO validation `33717be` | 1 passed | Installed-wheel linker round trip |
| OTIO demo `99a7558` | 1 passed | Installed-wheel Manager/linker round trip |
| Python host `bc7e568` | 1 passed | Installed-wheel persisted reference and resolution |
| C++ NLE baseline | 1 passed | Installed CMake consumer |
| Natron `017088c` | 1 passed | Installed native contract and CPython stable-ABI module build |
| Ardour `51e459c` | Passed | Installed resolver scenario; current patches replay on pinned source |
| Kdenlive `6205308` | Compiled | Sidecar with host compiler flags; current patches replay on pinned source |

The neutral Python wheel, built at SDK `dfb264a`, has SHA-256
`ccd191aea4d77b76d91a7cf1658099e67e03d91757e4946946aaaa04d49229ff`.
Its Python sources match the checkpoint above. Native C++ checks use the
installed `1a7c0cb` headers/library. Full native-host execution, Blender's 0.7
bundle, handoffs and final platform qualification remain pending.

## ABI 39 development checks

All 13 maintained checkouts remain on local `main`; 11 have implementation
commits. No origin push, tag or publication occurred. SDK `734511e` is
0.7.0-alpha.1 / Python 0.7.0a1 / ABI 39 / schema 17. Production ID signatures
are typed; coherent resolution, verification and portable decision tokens are
available. Other identity families and job leases remain incomplete.

The installed neutral wheel at this checkpoint has SHA-256
`4244ad57e9bd0134f472ea4efc445fcde03cb4ce2db05bdb9758f497f1f89bd1`.
The matching local Linux platform wheel has SHA-256
`c72570035255c7efe90985e5b27a6372318f264e62162c9721ac88819f49458f`.
These are local development artifacts, not published packages.

| Consumer commit | Executed result | Scope |
|---|---|---|
| Manager `662de75` | 6 passed | Installed neutral wheel, Python 3.13 |
| OpenAssetIO `a362568`, OTIO `33717be`, demo `99a7558`, Python host `bc7e568` | 1 passed each | Same wheel/library; separate repository test runs |
| Blender code `b01db7b`, tests `4140bac` | 23 passed, 1 skipped | Installed extension, Blender 5.2.2 LTS; 5.3 project-variable case skipped |
| Kdenlive `9f65738` | Compiled | Shared scenario with host flags; added patch applies to current pinned tree |
| OBS `930792a` | 2 passed; plugin/driver built | Installed C adapter contracts; no real-host execution |
| Natron `16fabb5` | 1 passed; module built | Installed native contract and stable-ABI CPython module |
| C++ NLE baseline | 1 passed | Fresh installed CMake consumer fixture |

OBS, Natron and the C++ NLE rows above used installed SDK `73dbc34`, before
the token additions. Their ABI 39 checks are recorded as earlier evidence;
they have not yet qualified the final candidate.

The Blender bundle SHA-256 is
`9c0de4b708abf78fada3d3680887076dde8ddc2185647c26d0852ce1130d829b`.
Its binding and native library come from the platform wheel above. Background
checks cover explicit save/render receipts, scoped relink conflicts,
wrong-production rejection and no-change results. The full cross-host scenario,
interactive checks and final platform qualification remain pending.

## ABI 40 development checks

SDK build `7635cf5` supplies the installed headers/library and neutral wheel
(`0.7.0a1`, ABI 40, schema 17). Wheel SHA-256:
`5b3df558409b07db52a7d59348ca021d286b97ef9af33d603a9d365d5cf77ce5`.
The artifact is `target/api-safety-wheel/journal-abi40/`; native installation is
`target/api-safety-install/`. SDK `3ba61b5` adds documentation without changing
that build's runtime.

| Consumer commit | Executed result | Scope |
|---|---|---|
| Manager `662de75` | 6 passed | Installed wheel, Python 3.13 |
| OpenAssetIO `a362568`, OTIO `33717be`, demo `99a7558`, Python host `bc7e568` | 1 passed each | Separate installed-wheel runs; OTIO uses the linker pin above |
| C++ NLE baseline | 1 passed | Fresh installed CMake consumer |
| Natron `85de6a4` | 1 passed; module built | Native contract and CPython stable-ABI module; no Natron GUI run |
| OBS `f5ef827` | 2 passed; plugin/driver built | Installed adapter contracts; no OBS host execution |
| Ardour `eb8be49` | Passed | Renamed/ambiguous stereo-WAV resolver scenario on the maintained patched source |

The installed SDK Python suite runs 63 tests with one skip. A separate Natron
bridge conflict scenario verifies typed revision text and retained conflict
sequences; its local record is `target/api-safety-natron-bridge40.json`.
Ardour's source is pinned at `7968ec504ba8b70e6de5c09d0470264581a5e979`
with maintained patches. Existing host checkouts were preserved.

Blender and Kdenlive evidence above still uses earlier SDK builds. These local
checks are not final coordinated host/platform qualification. Every maintained
repository remains on local `main`; no push, tag or publication has occurred.

## Job-status development checks

SDK `a56c0f1` adds checked C job payloads and C++/Python status alternatives,
still using ABI 40/schema 17. Its neutral wheel at
`target/api-safety-wheel/job-status/` has SHA-256
`003bcc45c52aefeb171727578ba11e335d84d1ec3563691959b5944acb13db7c`.
The installed prefix contains the new library and headers. Python 3.13 runs
66 installed-wheel tests with one skip; Manager `662de75` passes its six
installed-wheel tests using derived job properties. Other downstream results
above predate this slice. No final coordinated qualification is claimed.

## Explicit transaction contexts

SDK bindings/examples at `98e1cc5` require explicit Python commits. The native
library/headers are unchanged from `a56c0f1` (ABI 40, schema 17). Package
metadata is refreshed at `0512987`. Artifact SHA-256 values:

- Neutral wheel: `bf5bc4b405e6326e94fb91fccb3bf53cf20cc05e91e44a568631af977cc2d7e5`.
- Linux platform wheel: `a3778bc7d108bde3ceda6bf6cdf88245d2fc193e4a791fcc17c171ffffa7bd00`.
- Blender bundle: `f9434a7e7b6a1143b087d205a72855507243c7b5b1690eea1c3c49ed8f3fc386`.

Wheels are in `target/api-safety-wheel/explicit-transactions/`; the bundle is
in `target/api-safety-blender/explicit-transactions/`.

| Consumer commit | Executed result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Installed neutral wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 passed each | Same wheel/library; separate runs, pinned OTIO linker |
| Blender `71f49b6` | 23 passed, 1 skipped | Installed extension, Blender 5.2.2 LTS `d13f752e3b9c`; 5.3-only case skipped |

The SDK's installed-wheel suite runs 68 tests with one skip, and its installed
Python quickstart passes. Blender's runtime already committed explicitly; its
shared-media fixture now does too. Background checks cover saved bindings,
ambiguity, render provenance, conflicts and disabled/missing-sidecar behavior.
No interactive or final cross-host/platform qualification is claimed. All
changes remain on local `main`; origins and published releases are unchanged.

## ABI 41 asset migration checks

Runtime SDK `26c2ec6` supplies the installed native library/header and the
neutral wheel in `target/api-safety-wheel/asset-abi41/` (0.7.0a1, ABI 41,
schema 17). Later `642bb71` adds asset-scope regression and evidence metadata.
Neutral wheel SHA-256:
`bf2c81942170d1076367b7fb691f392ff12a90423de0da0ace4ec28bd6339c8c`.
Linux platform wheel SHA-256:
`4ad236a76ab03ee01c3ad77f467d8050f7d1eff442671c90a0e963b802574ab4`.
Blender bundle SHA-256:
`9e842cbde95a69ca130385b7894df05da929e0fe04083dcc055195cf907cd9ab`.

| Consumer commit | Executed result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Installed neutral wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 passed each | Separate installed-wheel runs; pinned OTIO linker |
| Blender `71f49b6` | 23 passed, 1 skipped | Installed bundle; Blender 5.2.2 LTS `d13f752e3b9c`; 5.3 project-variable case skipped |
| OBS `6c258e0` | 2 passed; plugin/driver built | Installed C contracts; checked reference projection; no host run |
| Natron `97b32d6` | 1 passed; module built | Installed native contract and CPython stable-ABI module |
| Ardour `e7a0c02` | Passed | Seven updated patches replay; renamed/ambiguous stereo-WAV resolver |
| Kdenlive `0312999` | Compiled | Ten updated patches replay; sidecar and shared/proxy tests compiled with real host flags |
| C++ NLE baseline | 1 passed | Fresh installed CMake consumer; version requirement still accepts 0.7 |

The Ardour/Kdenlive checks use fresh detached source worktrees and preserve
older fixtures and host checkouts. Full native hosts, interactive/background
handoffs and platform qualification remain pending. All maintained repos remain
on local `main`; no origin push, tag or publication occurred.

## Metadata decision checks

SDK `a73598b` retains ABI 41/schema 17. Native library/header inputs are
`84650bf`; Python inputs are `191c8c4`. Installed files use the separate
`target/api-safety-metadata-install` prefix, preserving earlier artifacts.
Artifact SHA-256 values:

- Neutral wheel: `d8e1be809ce5deaf9161d389e12b369c4b4adb2fa2e626314e76115ba9a073c7`.
- Linux platform wheel: `51acef60504bbf59b89d8434a14b27bb9ff8f7e67d3dbb3e45c234f1914ecfe2`.
- Blender bundle: `7eaef66d00f0f1c7bd94f96315e0d4c05d28d27e61908e3ba2cfa75fa3160645`.

Wheels are in `target/api-safety-wheel/metadata-abi41/`; the bundle is in
`target/api-safety-blender/metadata-abi41/`.

| Consumer commit | Executed result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Installed neutral wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 passed each | Separate installed-wheel runs; pinned OTIO linker |
| Blender `71f49b6` | 23 passed, 1 skipped | Rebuilt installed bundle; Blender 5.2.2 LTS `d13f752e3b9c`; 5.3-only case skipped |

Source and installed-wheel Python each run 73 tests with one skip. All six
required Rust gates, eight installed native contracts, 64 extracted tests,
strict docs, symbol/layout checks and installed typing fixtures pass. The
bundled platform-wheel quickstart passes without native-library overrides.
The dependency graph declares no Rust floor above 1.85, but the local compiler
is 1.98.1; exact MSRV execution remains pending.

Maintained native-host evidence remains the preceding asset checkpoint; those
pilots were not rebuilt for this metadata slice. Interactive host/handoff and
final platform qualification remain pending. All repositories stay on local
`main`, with no push, tag or publication.

## ABI 42 media-root checks

SDK `50a6027` contains native library inputs `67e2c98`, C++ header inputs
`b70476d` and Python inputs `7cba1c9`. Later commits change recipes, evidence
and static fixtures. Installed files use `target/api-safety-root-install`.
Schema 17, 294 exports and 22 public layouts agree. Artifact SHA-256 values:

- Neutral wheel: `6b0457a8e28ca8cf4767ba8c3dabaefc9ce3c9bb0f53493f2f88f23a54b4586f`.
- Linux platform wheel: `1317e12e5cd35a0cb9d3528dd0cd122ce5109ef2190d997083f06d8cfe35d500`.
- Blender bundle: `b5a1efc51bcc67ed10d95b8ebcc20edc622c108a9463658d117fa89006ee344b`.

Wheels are in `target/api-safety-wheel/root-abi42/`; the bundle is in
`target/api-safety-blender/root-abi42/`.

| Consumer commit | Executed result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Installed neutral wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 passed each | Separate installed-wheel runs; unchanged OTIO linker pin |
| Blender `71f49b6` | 23 passed, 1 skipped | Rebuilt installed bundle; Blender 5.2.2 LTS `d13f752e3b9c`; 5.3-only case skipped |
| Natron `5fd11c1` | 1 passed; module built | Native contract and stable-ABI module; actual bridge conflict text tested on Python 3.13 |
| OBS `2d957a6` | 2 passed; plugin/driver built | Installed direct-C contracts; SDK version documentation updated |
| Ardour `e7a0c02` | Passed | Stereo-WAV resolver rebuilt against ABI 42; unchanged maintained patched source |
| Kdenlive `0312999` | Compiled | Sidecar and shared/proxy tests rebuilt with host flags from the preserved patched source |
| C++ NLE baseline | 1 passed | Fresh installed CMake consumer |

All six required Rust gates, eight native contracts, 64 extracted tests, strict
docs and installed typing fixtures pass. Source and installed-wheel Python
each run 75 tests with one skip. The bundled-wheel quickstart runs without
native-library overrides. Natron's bridge scenario records its structured
conflict in `target/api-safety-root-natron-bridge.json`. No full native-host,
interactive/handoff, exact MSRV or final platform qualification is inferred.
All changes remain on local `main`; no push, tag or publication occurred.

## Root decision checks

SDK runtime `a6e3907` and source manifest `b66cf7e` retain ABI 42/schema 17.
The separate `target/api-safety-root-edits-install` prefix contains the guarded
root edits. Neutral wheel SHA-256:
`79ee0977ffdd80cf0962a511e73d9c587394e081ff33276a5df0a11ae9bc6071`;
path: `target/api-safety-wheel/root-edits-abi42/`.

All six Rust gates, eight installed native contracts, 64 extracted tests,
strict docs and symbol/layout checks pass. Source and installed-wheel Python:
77 run, one skip. Installed typing rejects five wrong-kind calls. This wheel
was not qualified against the maintained hosts; their evidence above uses the
preceding candidate. No publication or final qualification is claimed.

## ABI 43 locator checks

SDK `2501860` uses native library inputs `63d4490`, C++ header inputs `1963028`
and Python inputs `c5f104a`. The separate `target/api-safety-locator-install`
prefix retains schema 17. Artifact SHA-256 values:

- Neutral wheel: `b75f206836b68f58143cfff5948bbf518dfd1430859d82352c605657ad5d2601`.
- Linux platform wheel: `8baec78d72c2e90fa636ee50adb550fca9cf291b044faed09694c85837de3293`.
- Blender bundle: `3305a066356eb60e5cc30a6aee8f5cb4a361187dd50a68095df96164b238051a`.

Wheels live in `target/api-safety-wheel/locator-abi43/`; the bundle is in
`target/api-safety-blender/locator-abi43/`.

| Consumer commit | Result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Installed neutral wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 passed each | Separate matching-wheel runs; unchanged OTIO linker pin |
| Blender `71f49b6` | 23 passed, 1 skipped | Rebuilt installed bundle; 5.2.2 LTS `d13f752e3b9c`; 5.3-only case skipped |
| Natron `68a0304` | 1 passed; module built | Installed native contract and actual Python 3.13 bridge-conflict regression |
| OBS `6c50cb0` | 2 passed; plugin/driver built | Installed direct-C contracts |
| Ardour `e7a0c02` | Passed | Rebuilt stereo-WAV resolver; preserved patched source |
| Kdenlive `0312999` | Compiled | Sidecar/shared-proxy translation units with real host flags; preserved patched source |
| C++ NLE baseline | 1 passed | Fresh installed CMake consumer |

All six Rust gates, eight installed native contracts, 64 extracted tests,
strict docs and symbol/layout checks pass. Source and installed-wheel Python:
79 run, one skip; six installed wrong-kind calls are rejected. The bundled
wheel quickstart passes without library overrides. No full native-host run,
interactive handoff, exact MSRV or final platform qualification is inferred.
All changes remain on local `main`; no push, tag or publication occurred.

## ABI 44 job identity checks

SDK `28e24d9` uses native library inputs `1072ac6`, C++ header inputs `c96356d`
and Python inputs `10d50f7`. Installed files use `target/api-safety-job-install`.
ABI 44/schema 17 has 300 exports and 24 agreeing public layouts. Artifact
SHA-256 values:

- Neutral wheel: `25dd71a0d1b5c8282663473d4e183ceed98c804a4d53526a2d6922b48300aff5`.
- Linux platform wheel: `8fca72ee99aae193365f84f9868118ab5c9e2ebbb3e54df7b963cc587613b00b`.
- Blender bundle: `57ee56b1b9d372ad48d47b708a37bb9a6e89561d07a2acc1fe14e7641a476f1a`.

Wheels are in `target/api-safety-wheel/job-abi44/`; the bundle is in
`target/api-safety-blender/job-abi44/`.

| Consumer commit | Result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Installed neutral wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 passed each | Separate matching-wheel runs; unchanged OTIO linker pin |
| Blender `71f49b6` | 23 passed, 1 skipped | Rebuilt installed bundle; 5.2.2 LTS `d13f752e3b9c`; 5.3-only case skipped |
| Natron `b48b40a` | 1 passed; module built | Installed native contract and actual Python 3.13 bridge-conflict regression |
| OBS `bc9297c` | 2 passed; plugin/driver built | Installed direct-C contracts |
| Ardour `e7a0c02` | Passed | Rebuilt stereo-WAV resolver from the preserved patched source |
| Kdenlive `925e5b8` | Replayed/compiled | Ten patches on pinned upstream; sidecar/shared-proxy units with real host flags |
| C++ NLE baseline | 1 passed | Fresh installed CMake consumer |

All six Rust gates, eight native contracts, 64 extracted tests, strict docs
and symbol/layout checks pass. Source and installed-wheel Python: 81 run, one
skip; eight installed wrong-kind calls fail typing. The bundled quickstart
passes without library overrides. Claims/time, full native hosts, interactive
handoffs, exact MSRV and final platform qualification remain open. All changes
are on local `main`; no push, tag or publication occurred.

## ABI 45 activity identity checks

SDK `eedfc35` uses native inputs `38b9fb5` plus utility documentation fixes,
C++ inputs `49dde64` and Python inputs `a833bdf`. Installed prefix:
`target/api-safety-activity-install`. ABI 45/schema 17 has 304 exports and
25 agreeing layouts. SHA-256 values:

- Neutral wheel: `fcbf90dcdeeded254eb1e5295b9f645e479951b6ac870df4a56fcf67ea8a634f`.
- Linux wheel: `4ceddd4bbfb489ac6491b013150fcdff7d5360f08b401bfe467f82ff52431613`.
- Blender bundle: `01b082d7d950cd094303956f56c5f65517d992a138ece85167108f205d7e616c`.

Wheels: `target/api-safety-wheel/activity-abi45/`; bundle:
`target/api-safety-blender/activity-abi45/`.

| Consumer commit | Result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Matching installed neutral wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 passed each | Separate matching-wheel suites; existing OTIO linker pin |
| Blender `71f49b6` | 23 passed, 1 skipped | Rebuilt bundle, 5.2.2 LTS `d13f752e3b9c`; 5.3-only case skipped |
| Natron `e1f4f15` | 1 passed; module built | Native contract plus actual Python 3.13 bridge conflict |
| OBS `7ede430` | 2 passed; plugin/driver built | Direct-C registration and readback |
| Kdenlive `0a0b04c` | Replayed/compiled | Ten patches on upstream `55e16e8`; sidecar/test units with host flags |
| Ardour `e7a0c02` | Passed | Installed stereo-WAV resolver from preserved patched source |
| C++ NLE baseline | 1 passed | Fresh installed CMake consumer |

All six Rust gates, eight native contracts, 64 extracted tests, strict docs
and symbol/layout checks pass. Source and installed Python: 83 run, one skip;
nine installed wrong-kind calls reject. The bundled quickstart passes without
library overrides. Local logs include `target/activity-{examples,sphinx,
blender}.log`, the bridge JSON and Kdenlive compilation logs. Full hosts,
interactive handoffs, exact MSRV and platform qualification remain open.
All changes remain on local `main`; no push, tag or publication occurred.

## ABI 46 representation identity checks

SDK `ec7c38a` uses native inputs `9d8828d`, C++ inputs `de7f820` and Python
inputs `69c4412`. Prefix: `target/api-safety-representation-install`.
ABI 46/schema 17 has 308 exports and 26 agreeing layouts. SHA-256 values:

- Neutral wheel: `34310f14d9cf26a5ecb652ad7eff8ab164193fdb8c6e30b8baea599354d61075`.
- Linux wheel: `aa908c8982c91a3df77f1a5bb54c27b34e52f928ce413fa9baaad43f3dd6dda1`.
- Blender bundle: `f759ab0ac6bbccef6aeda5e3ee7e7c2a3723dbf05c9b55f4c3638198df7f1d4a`.

Wheels: `target/api-safety-wheel/representation-abi46/`; bundle:
`target/api-safety-blender/representation-abi46/`.

| Consumer commit | Result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Matching installed wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 passed each | Separate matching-wheel suites; existing OTIO linker pin |
| Blender `71f49b6` | 23 passed, 1 skipped | Rebuilt bundle, 5.2.2 LTS `d13f752e3b9c`; 5.3-only case skipped |
| Natron `b33afd2` | 1 passed; module built | Native contract plus actual Python 3.13 bridge conflict |
| OBS `eb26ad6` | 2 passed; plugin/driver built | Direct-C registration/readback |
| Kdenlive `10c5445` | Replayed/compiled | Ten patches on upstream `55e16e8`; sidecar/test units with host flags |
| Ardour `e7a0c02` | Passed | Installed stereo-WAV resolver from preserved patched source |
| C++ NLE `8d2933a` | 1 passed | Fresh installed CMake consumer |

Six Rust gates, eight native contracts, 64 extracted tests, strict docs and
symbol/layout checks pass. Source/installed Python: 85 run, one skip; nine
installed wrong-kind calls reject. The bundled quickstart passes without
library overrides. Logs use `target/representation-*.log`; bridge JSON and
Kdenlive compilation logs are retained. The compatibility manifest updates
changed-family dependency closure. Full hosts, handoffs, exact MSRV and final
platform qualification remain open. Changes are local `main` commits;
no push, tag or publication occurred.

## ABI 47 resource identity checks

SDK `1ce127e` uses native operation inputs `b9e545d`, C++ `8dc8082`, Python
`e41e501` and import/fixture fixes `4b81cb2`. Installed prefix:
`target/api-safety-resource-install`. ABI 47/schema 17: 312 exports, 27 layouts.
Artifacts in `target/api-safety-wheel/resource-abi47/` and
`target/api-safety-blender/resource-abi47/` have these SHA-256 values:

- Neutral wheel: `4141a11cd2aeba993c1730e86124f5699b4be99a50e6cebf22bbb085841ed268`.
- Linux wheel: `730e72f25747b3796e929a589e27f064fdca6c842e5c05e3c91f4b8bdaa5659f`.
- Blender bundle: `20b1a8a83e458125298a85f3f46ec12c5014d25a6088f6a7807d453a6cba7170`.

| Consumer commit | Result | Scope |
|---|---|---|
| Manager `da2caaa` | 6 passed | Matching wheel, Python 3.13 |
| OpenAssetIO `5e38c31`, OTIO `81a6ee6`, demo `c18f5bc`, Python host `c42b8b8` | 1 each | Separate matching-wheel suites; pinned OTIO bridge |
| Blender `71f49b6` | 23 passed, 1 skipped | Rebuilt bundle; 5.2.2 LTS `d13f752e3b9c`, 5.3-only skip |
| Natron `7192a5c` | 1 passed; module built | Native contract and actual Python 3.13 bridge conflict |
| OBS `fe68bcc` | 2 passed; plugin/driver built | Direct C registration/readback |
| Kdenlive `956bcbf` | Replayed/compiled | Ten patches on `55e16e8`; two units with host flags |
| Ardour `535ba87` | Passed | Fresh seven-patch replay on `7968ec504`; stereo-WAV resolver |
| C++ NLE `8d2933a` | 1 passed | Fresh installed CMake consumer |

Six Rust gates, eight native contracts, 64 extracted tests, strict docs and
symbol/layout checks pass. Source/installed Python: 87 run, one skip; nine
installed wrong-kind calls reject. Bundled quickstart passes without overrides.
Logs use `target/resource-*.log`; host compilation logs, detached qualification
trees and bridge JSON are retained. Full hosts, handoffs, exact MSRV and final
platform checks remain open. All task changes are local `main` commits; no
push, tag or publication occurred.

## Locator retirement checks

SDK `380b57e` (storage `9e2a374`, native `c8a8368`, CLI `f442ba1`)
retains ABI 47/schema 17. Fresh prefix:
`target/api-safety-locator-decisions-install` (debug CLI, release native
library; no release-artifact claim). Wheels in
`target/api-safety-wheel/locator-decisions/`, bundle in
`target/api-safety-blender/locator-decisions/`, SHA-256:

- Neutral wheel: `04c39b07091db548a3da73aaafd1ca09a88e08d17414de8b346ae6bc6a603286`.
- Linux wheel: `74d2076c4d488068c458605999d359e258fa277a972139e5c5ca321b2e6d0520`.
- Blender bundle: `abacf7c8be3c5172295797ef6916a63826cadbf439a1c2b600d5659cc29d477d`.

Six Rust gates, eight native tests, 64 extracted tests, strict docs, Ruff/ty
and 20 tooling tests pass. Source/installed Python: 88 run, one skip; nine
wrong-kind calls reject. Blender `71f49b6`: 23 passed, one 5.3-only skip on
5.2.2 LTS `d13f752e3b9c`. Kdenlive `a9c2004`: fresh ten-patch replay on
`55e16e8`, then the eleventh patch; both affected units compile with host flags.
Logs: `target/locator-decisions-*.log` and
`target/api-safety-kdenlive-locator-decisions-*.log`. Other consumer results
remain scoped to their earlier inputs; no full-host or final-candidate claim.
All task commits remain local `main`; no push, tag or publication occurred.

## External identifier removal checks

SDK `4c64491` (storage `d2144cf`, native `172134d`, CLI `dcc248f`)
retains ABI 47/schema 17. Prefix: `target/api-safety-identifier-decisions-install`
(debug CLI, release native library). Source binding code is unchanged; the
neutral wheel from the locator checkpoint was tested against this prefix.
The fresh Linux wheel in `target/api-safety-wheel/identifier-decisions/`
has SHA-256 `17a7177a79576dcb1dd0db1dfa4b50228b5dadc5cc0da1cef257ab9304cd5c29`.
Six Rust gates, eight native contracts, 64 extracted examples, strict docs,
Ruff/ty and 20 tooling tests pass. Source/installed Python: 89 run, one skip;
nine wrong-kind typing calls reject. Logs: `target/identifier-decisions-*.log`.
The maintained hosts do not call identifier removal; their earlier results
retain their original input scope. Final qualification remains open.
All task commits remain local `main`; no push, tag or publication occurred.
