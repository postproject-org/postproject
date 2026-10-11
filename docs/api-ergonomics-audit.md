# API ergonomics audit

Baseline: released `v0.8.0-alpha.1`, `7e097bd`, inspected 2026-10-11.
Python is `0.8.0a1`, C ABI 52 has 349 exports and 36 layouts, schema is 27,
and the experimental exchange protocol is major 1. This is a development
record; proposed interfaces below are not released APIs.

## Reconciled starting point

The release exists with 18 assets. All six release jobs and all sixteen tag
CI jobs pass. Both documentation jobs pass; live deployment verification is
separate. The final publication checkpoint in the external planning brief
supersedes its earlier unpublished snapshot. The qualified 0.8 reports and
archives remain historical evidence.

The [repository inventory](api-ergonomics-repositories.json) records all 13
active GitHub repositories, local starting commits and unrelated untracked
builds. All start on `main`. The unselected Shotcut checkout has no origin
and is outside the active inventory. No new repository is needed initially.

## Full workflow audit

These costs count visible API setup, not fixture creation or assertions.
Each linked source remains the baseline until its migration is recorded.

| Task | Current caller | Avoidable cost | Contract to retain |
|---|---|---|---|
| Import | Python host `host.py:exercise`; Python `lifecycle.py` | Separate additive transaction and decision edit entry points; revision-context object outside the keyword path | Explicit commit, rollback, terminal failures and own receipt |
| Adopt | Blender `tests/shared_production.py`; OBS `tests/handoff_blender.py` | `tuple(production.assets)` eagerly copies the entire asset set, even to check one fixture | Existing identity, ambiguous candidates and explicit selection |
| Resolve moved media | Natron `src/resolution.cpp`; C++ `ResolutionOptions` | Empty factory plus independently checked mapping, verification, bounds and cancellation setters | Atomic validation, cancellation and no implicit confirmation |
| Record derived output | Blender render integration; Manager `manager.py` | Property and scalar metadata wrappers around already native values | Exact kinds, ordered/repeated values and atomic provenance |
| Consume revisions | Python `read_sessions.py`; `changes_since` | Caller maintains sequence and page loops | Pinned view, bounded memory, gaps/errors and deterministic close |

The Python asset collection introduces a private facade, eager iteration,
eager length and special membership. A bounded asset scan already exists as
`assets_page`; its caller maintains a cursor and checks continuation after
every page. A pinned `ReadSession` already owns the consistency boundary.
Replacing the facade with a session iterator removes one collection concept
and the three cursor-management steps. Explicit paging remains useful.

## Selected first slice

`ReadSession.assets(page_size=100)` will return a lazy iterator over bounded
pages. The caller explicitly owns the session. Creation validates the page
size and session; every advance checks that the session is open, including
when a page is buffered. Copied assets survive close. No implicit connection,
total cap, partial-success result or cancellation token is introduced for a
knowledge-only scan. Closing the view ends further iteration.

Remove `Production.assets` and its eager implementation. Use
`Production.asset_exists(id)` for membership and `view.assets()` for scans.
Migrate SDK tests/examples and real OpenAssetIO, Blender, OBS and Natron
callers. Preserve low-level production/session page queries. Required checks
include page boundaries, concurrent writer isolation, close before/after
buffering, invalid sizes and native failure propagation through an installed
wheel. The migration is incomplete until those consumers pass.

## Remaining design and qualification

Choose one edit entry point for additive, retained and detached decisions;
native metadata inputs with explicit ambiguous alternatives; atomic C++
configuration; and task-oriented Rust/C/CLI recipes. These are pending design
and implementation, not delivered functionality. Do not bump schema or
protocol for source-only changes. Set package/ABI/JSON versions when their
actual breaks are known, then reconcile every consumer before release.

Every active repository needs a meaningful migration: SDK and sample hosts
exercise canonical APIs; adapters/Manager/demos update calls and dependency
bounds; Blender/OBS/Natron migrate host paths and handoffs; Kdenlive/Ardour
replace maintained patches and verify optional builds; the site exposes
integrations, reproducible scenarios and accurate installation states.
Distribution packets, clean installs, both real handoffs and full candidate
qualification remain pending. No public release, deployment or upstream
message is performed by this audit.
