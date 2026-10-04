# API safety repository inputs

Inventory checked against `gh repo list postproject-org` on 2026-10-04.
All 13 non-archived organization repositories have local checkouts; the
12-repository release evidence excludes the landing site. All start on `main`.
Only untracked build directories were present. Candidate migration and
host/platform verification are pending for every row.

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
