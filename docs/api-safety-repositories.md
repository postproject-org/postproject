# API safety repository inputs

Inventory checked against `gh repo list postproject-org` on 2026-10-04.
All 13 non-archived organization repositories have local checkouts on `main`;
the historical 12-repository evidence excludes the landing site. Local checks
use 0.7.0-alpha.1 / Python 0.7.0a1, C ABI 51 and schema 19. Remote CI and
full host/package checks remain open.

| Repository | Baseline SHA | Candidate SHA / surface | Local result and remaining scope |
|---|---|---|---|
| [postproject](https://github.com/postproject-org/postproject) | `a413227b9dd048f744fa7393d8927891ee93d5e2` | `ebd97d5` / Rust, C, C++, Python, CLI | Six Rust gates, nine native contracts and sanitizer tests, 121 Python tests (one skip), 72 extracted checks, strict docs; platform/MSRV/Flatpak verification open |
| [postproject-ardour](https://github.com/postproject-org/postproject-ardour) | `7ec34c2a972b936191494f25d420c4e962f8c047` | `a417d20` / C++ / pkg-config | Five replacement patches replay; stereo-WAV resolver scenario passes; full Linux host and macOS package CI open |
| [postproject-blender](https://github.com/postproject-org/postproject-blender) | `8c62d4ca6243446219ce82383dd3ed823f850c3c` | `bfe5b95` / Python extension | 5.2.2 LTS: 24 tests run, one 5.3-only skip; OBS handoff passes; other platform and shared-host checks open |
| [postproject-cpp-nle](https://github.com/postproject-org/postproject-cpp-nle) | `5d801c0b59526a098b9111beeb40b143bbe59ca1` | `8d2933a` / Installed C++ host | One CTest passes against the installed candidate; other platform CI open |
| [postproject-kdenlive](https://github.com/postproject-org/postproject-kdenlive) | `2eb45f9019cdb48fdc35606768c9ecc164943b8f` | `33fc107` / Native C++ / Result | Ten replacement patches replay; Qt thumbnail lint/runtime checks pass; fresh full build and shared handoff running/open |
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
