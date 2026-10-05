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

The Blender bundle SHA-256 is
`9c0de4b708abf78fada3d3680887076dde8ddc2185647c26d0852ce1130d829b`.
Its binding and native library come from the platform wheel above. Background
checks cover explicit save/render receipts, scoped relink conflicts,
wrong-production rejection and no-change results. The full cross-host scenario,
interactive checks and final platform qualification remain pending.
