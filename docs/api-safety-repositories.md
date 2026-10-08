# API safety repository inputs

Inventory checked against `gh repo list postproject-org` on 2026-10-04.
All 13 non-archived organization repositories have local checkouts on `main`;
the historical 12-repository evidence excludes the landing site. Local checks
use 0.7.0-alpha.1 / Python 0.7.0a1, C ABI 51 and schema 19. Required SDK/platform/package and downstream CI jobs pass. Kdenlive’s
Qt 6.12 full-host lint gate remains open; Blender’s optional 5.3-alpha
download is unavailable.

| Repository | Baseline SHA | Candidate SHA / surface | Local result and remaining scope |
|---|---|---|---|
| [postproject](https://github.com/postproject-org/postproject) | `a413227b9dd048f744fa7393d8927891ee93d5e2` | `42b9f9d` / Rust, C, C++, Python, CLI | Six Rust gates, nine native contracts and sanitizer tests, 121 Python tests (one skip), 72 extracted checks, strict docs; Rust 1.85, Linux/macOS/Windows packages, manylinux 2.28 and Flatpak CI pass |
| [postproject-ardour](https://github.com/postproject-org/postproject-ardour) | `7ec34c2a972b936191494f25d420c4e962f8c047` | `a417d20` / C++ / pkg-config | Five replacement patches replay; stereo-WAV resolver scenario passes; full Linux with/without and macOS resolver CI pass |
| [postproject-blender](https://github.com/postproject-org/postproject-blender) | `8c62d4ca6243446219ce82383dd3ed823f850c3c` | `bfe5b95` / Python extension | 5.2.2 LTS: 24 tests run, one 5.3-only skip; OBS handoff passes; required 5.2.2 CI and shared-host handoff pass; optional 5.3 download unavailable |
| [postproject-cpp-nle](https://github.com/postproject-org/postproject-cpp-nle) | `5d801c0b59526a098b9111beeb40b143bbe59ca1` | `8d2933a` / Installed C++ host | One CTest passes against the installed candidate; own and SDK downstream CI pass |
| [postproject-kdenlive](https://github.com/postproject-org/postproject-kdenlive) | `2eb45f9019cdb48fdc35606768c9ecc164943b8f` | `322aa7a` / Native C++ / Result | Ten replacement patches replay; full local build, document/pilot tests and shared handoff pass; Qt 6.12 CI lint fails |
| [postproject-natron](https://github.com/postproject-org/postproject-natron) | `4694e97a65ab59954a3ef32c7984ed6ccd53a97f` | `ad2554d` / C++ / CPython stable ABI | Native contract, request-generation tests and Natron 2.5.0 normal/negative/plugin-free renderer paths pass on Linux |
| [postproject-obs](https://github.com/postproject-org/postproject-obs) | `5c15a4a7fc39d4e22d191c3fc854915f098931c9` | `338e395` / C11 / Qt frontend | OBS 32.2.2: two contracts, eight isolated host scenarios and Blender adoption/move/plugin-free reopen pass on Linux |
| [postproject-openassetio](https://github.com/postproject-org/postproject-openassetio) | `99ab45e38dc31b2ff5f8e63cd9a848f090cc389d` | `5e38c31` / Python validation host | One installed-wheel pytest passes; declared 0.7 range updated |
| [postproject-openassetio-manager](https://github.com/postproject-org/postproject-openassetio-manager) | `796631907817695cf33ca6a0276bb243bc54a099` | `ed5293c` / Python Manager / jobs | Six installed-wheel tests pass with authority-timed publishing leases |
| [postproject-otio-demo](https://github.com/postproject-org/postproject-otio-demo) | `bb67d3483acb0244906d35692bd1304ff2966a80` | `c18f5bc` / Manager + OTIO / Python | One installed-wheel/linker round-trip test passes; Manager 0.4 range updated |
| [postproject-otio-openassetio](https://github.com/postproject-org/postproject-otio-openassetio) | `afcd6503f5286b0ce3fa2ae4165340f8ef86b690` | `81a6ee6` / OTIO/OpenAssetIO / Python | One installed-wheel/linker round-trip test passes; declared 0.7 range updated |
| [postproject-python-host](https://github.com/postproject-org/postproject-python-host) | `98e7ee184f6a795bd42c99d331724157fd6235dc` | `c42b8b8` / Installed Python host | One persisted-reference/resolve test passes against the installed wheel |
| [postproject.org](https://github.com/postproject-org/postproject.org) | `606f228b61adefe1748b029b72228ec549dd37cb` | `606f228` / Landing site | Three site checks pass; released 0.6 links retained because 0.7 is unpublished |

The local `postproject-shotcut` repository has no remote and contains the
previously rejected pilot scaffold. It is historical, not a newly selected
integration. No Audacity, Krita or GES/Pitivi integration was found.

All checkouts inherit the workspace `AGENTS.md`; no nested `AGENTS.md` was found.
No new GitHub repository is needed for these local API changes. Publication of
a release tag requires separate explicit authorization.

Candidate inputs and commands are recorded below. Local results do not qualify
a different platform or a later runtime change.

## Installed candidate

The Linux release library, static library and CLI were rebuilt at `b25e082`
into `target/qualification-abi51-install`. Later commits change tests,
verification tools and documentation. Both installed headers match current
source byte for byte; all 322 exported symbols match the manifest.

| Artifact | SHA-256 |
|---|---|
| `libpostproject.so` | `cc0e7cf1a36ff20a34256f360c81eaef713e747d4803ecf41a198faf80c397e5` |
| `libpostproject.a` | `cc2932f7ab9ca45f761311dd8ccf0ec5fba0ac7d9cb25f1ad332bc9d706e6f68` |
| CLI `postproject` | `5020a75746a538f596c0e8865f36bef730daa3d5441f4073d6c042654841df1e` |
| Neutral wheel `postproject-0.7.0a1-py3-none-any.whl` | `aecbf4e0aff301e8382a6e91345665800caef50dff7a3a568bd34ee514e2f3dc` |
| Local wheel `postproject-0.7.0a1-py3-none-linux_x86_64.whl` | `05a73600074aabf13501d0c7658efae61bf4091da54a466c67b693ed8b442818` |
| Blender extension `postproject_relink-0.1.0-linux_x64.zip` | `71f6e63724d1216d5cc7366d1d610bdfe9f4d08a6f69a33f813a71951ba9e2eb` |

The neutral wheel's nine Python source files match current source. These are
local review artifacts; the Linux wheel makes no manylinux claim. CI separately
builds and tests the glibc 2.28 wheel. Packages remain unpublished.

## Reproduction and local evidence

Run the six required Rust gates from the workspace instructions, followed by
the installed consumer commands in {doc}`testing`. Candidate native tests use
`target/qualification-abi51-native`; extracted examples use
`target/qualification-abi51-examples`. Python tests use the installed neutral
wheel with `POSTPROJECT_LIBRARY` and `POSTPROJECT_CLI` pointing into the prefix.
`tools/check_python_id_typing.py --python VENV/bin/python --ty VENV/bin/ty`
checks installed typing metadata, positive examples and nine negative cases.

Strict Doxygen/Sphinx, offline links and spelling pass. Exact Rust 1.85.0
checks all workspace targets/features. The installed C/C++ contracts pass with
ASan/UBSan; the Rust library is not sanitizer-instrumented. Five nightly
libFuzzer/AddressSanitizer campaigns ran at least 60 seconds each without a
finding. All six Criterion workloads pass, including a coherent, paged
10,000-asset read. No hard latency threshold or published timing claim applies.

Kdenlive's ten patches replay on `55e16e85cd9a9c6e032cd27a621137b4da881a7c`;
Ardour's five replay on `7968ec504ba8b70e6de5c09d0470264581a5e979`.
Fresh enabled Kdenlive compilation and both document/pilot tests pass. The
shared-production script passes with Blender 5.2.2 LTS (`d13f752e3b9c`), the
rebuilt extension and the installed Manager under Python 3.13. Its retained
workspace is `/tmp/tmp.2sMCDlF6KV`; traces are in
`target/qualification-abi51-final-shared-traces`. OBS and Natron acceptance retain
their declared Linux host scope.

Local logs and artifacts are retained under `target/qualification-abi51-*`.
Ardour’s replay reproduces the current series exactly. Kdenlive’s final
replay is pending the Qt 6.12 lint fix. Host tests isolate
configuration/cache; they do not claim an interactive human acceptance pass.

## GitHub qualification

At SDK `42b9f9d`, every individual job passes in [SDK CI](https://github.com/postproject-org/postproject/actions/runs/37789355697)
(16 jobs), [downstream CI](https://github.com/postproject-org/postproject/actions/runs/37789356724)
(10 jobs), and [documentation deployment](https://github.com/postproject-org/postproject/actions/runs/37789355708).
[Flatpak](https://github.com/postproject-org/postproject/actions/runs/37780003984)
passes at `6f14a3b`; subsequent SDK changes affect tests, tools and docs.

Consumer push jobs pass for Manager, both validation experiments, Python host,
C++ NLE, Ardour, OBS, Natron and the OTIO demo. Blender’s required lint/5.2.2 jobs
pass; its optional 5.3-alpha job fails while downloading the unavailable host.
[Kdenlive’s full-host jobs](https://github.com/postproject-org/postproject-kdenlive/actions/runs/37789160505)
fail in Qt 6.12 lint at `322aa7a`, before compilation and host tests.
