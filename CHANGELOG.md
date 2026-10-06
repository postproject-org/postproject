# Changelog

All notable changes to PostProject will be documented here. The project uses
[Semantic Versioning](https://semver.org/) once a stable API is released.

## Unreleased

- Make C++ object references private typed variants with checked interchange and projections.

- Validate Python query, media, metadata and timestamp integers before native conversion, rejecting wraparound.

- Remove cached Rust production roots; open/edit stay bounded and CLI inspection pages roots.

- Started the `0.7.0-alpha.1` development series; consumer builds require matching SDK artifacts.
- Added coherent reads, scoped decisions, explicit edits and atomic receipts; Python transaction contexts require `commit()` (ADR 0045, C ABI 38).
- Replaced Python ID wrappers with UUID NewType hints and explicit object-reference variants; use IDs directly and wrap dynamic targets (ADR 0046).
- Bound query cursors to their production and retained read view; restart old cursors and page revision events (ADRs 0047, 0050).
- C++ tokens/options require `create()`; options setters return `Result<void>` immediately and preserve valid settings on failure (ADR 0048).
- Added native production/asset/media-root/locator/job/activity/representation/resource/revision/transaction IDs (C ABI 47); persisted references retain their meaning (ADR 0049).
- Job reads use checked C payloads and one C++/Python status alternative; migrate C++ field access and Python construction (ADR 0051).
- Reject oversized native binary metadata and unaddressable resolution arrays before access.
- Metadata appends merge and invalidate stale destructive decisions; replacement/removal require a decision base (ADR 0052).
- Root enabling/removal, locator retirement and identifier removal require a decision base (ADR 0045).
- Complete dependency observations require a decision base, including initial and unchanged sets (ADR 0045).
- Add fresh bounded media-root pages; convenience reads cap at 1000 roots (ADR 0053).
- Validate Python root flags and signed priorities before native conversion.
- CLI JSON emits structured conflicts on stdout and commit receipts for media/confirmation, metadata, roots, identifiers, dependencies, activity and job writes.

## 0.6.0-alpha.1 - 2026-10-04

- Named the C++ Result propagation protocol as source-compatible within `0.6.x` (ADR 0020).
- Fixed release of closed transaction handles clearing a newer transaction's guard; no caller migration is required.
- Added installed C OBS and native C++ Natron pilots, host-action recipes, and complete compatibility-family evidence checks.
- Enforced crate dependency boundaries and checked Rust/C/ctypes struct layouts on all native CI platforms.

## 0.5.0-alpha.1 - 2026-10-01

- Added opt-in C ABI usage traces and mechanical compatibility-family reports.
- Added bounded current-locator and resource-fingerprint lookup for explicit known-media adoption (ADR 0041, C ABI 36, schema 16).
- Added optional base revisions and structured semantic write conflicts (ADR 0042, C ABI 37, schema 17).
- Confirmed that 0.5 names no compatibility subset after the usage-evidence review (ADRs 0020 and 0040).

## 0.4.0-alpha.1 - 2026-09-29

### Migrating from 0.3

The integration-preview subset of ADR 0020 ended with the `0.3.x` series; every
API of `0.4.x` is experimental. These changes affect code written against the
`0.3.x` subset:

- `pp_production_resolve_asset` is replaced by `pp_production_resolve_assets`,
  which takes an array of assets and a nullable `pp_resolution_options_t`
  holding the root mappings, search directories, verification mode, budgets,
  and cancellation token. C++ `resolveAsset` takes a `ResolutionOptions`
  instead of root mappings, and Python `Production.resolve` accepts one asset
  or several.
- `pp_resolution_set_get_representation` also returns the asset, and
  `pp_resolution_set_get_candidate` the candidate's media root and sequence
  naming.
- An image sequence's prefix, suffix, and padding moved from
  `pp_representation_set_get_sequence` to `pp_representation_set_get_locator`,
  and from the C++ and Python sequence structure to each locator's
  `SequenceNaming`. Missing frames are read with
  `pp_representation_set_get_sequence_missing_frame`.
- `pp_transaction_confirm_locator` takes a nullable media-root name and a
  sequence naming, which an image-sequence resource requires and any other
  resource must leave `NULL`; pass the naming the chosen candidate reports.
- Release a formatted host-object reference with `pp_string_release` instead
  of `pp_host_binding_release`.
- C++ subset operations return `postproject::Result<T>`; call `value()` to keep
  throwing, or test the result when exceptions are disabled.
- Productions from any 0.3 release migrate to SQLite schema 15 when opened.
  Version 1 sequence fingerprints are no longer computed; observe the sequence
  again before comparing it.

### Added

- Added Python platform wheels that carry the native library, and built the
  Linux package and wheel for glibc 2.28 (ADR 0039).
- Added conversion between paths and canonical locator URIs on every surface
  (ADR 0030, C ABI 31).
- Added content fingerprinting, verification, and observation that recomputes
  every representation using a resource, on every surface (ADR 0029, C ABI 29).
- Added a tested Flatpak module that builds PostProject offline from the release
  source archive, published with its Cargo sources on each release.
- Documented publishing media through the OpenAssetIO Manager (ADR 0010).
- Added Mermaid diagrams of the model, integration depths, relinking, jobs,
  revisions, and artifact evaluation to the documentation.
- Added point reads for assets, representations, and jobs, and a page of the
  representations using a resource, on every surface (C ABI 27).
- Added tested documentation examples for every public operation. Every C
  function, C++ member function, and Python method, every CLI command, and the
  Rust storage and media services now appear in a program that CI runs, shown
  as synchronized C, C++, Python, Rust, and CLI tabs in the guides. New guides
  cover fingerprints, verification, and inventory; dependencies; and artifacts
  and staleness. `tools/check_example_coverage.py` keeps the coverage complete.
- Added `postproject revisions filtered --kind <event_kind>`, which prints the
  matching revisions and the `through_sequence` cursor, and `postproject
  revisions wait`, which blocks for at most `--timeout-ms` (default and maximum
  60000) until revisions after `--after` (default: the latest revision) exist
  and prints the result as `revisions`, `timed_out`, `closed`, or `cancelled`.
- Added change delivery to Python: `Production.changes_since_filtered` over
  event payload classes, `Production.revision_waiter()` with a GIL-releasing
  `wait()` and thread-safe `cancel()`, and a `RevisionObserver` that calls back
  on a thread it owns. A test observes a proxy completed by a CLI `job run` in
  another process.
- Added change delivery to the C++17 wrapper: `changesSinceFiltered` with a
  `RevisionEventKind` enum, a move-only `RevisionWaiter` with thread-safe
  `cancel()`, and a `RevisionObserver` that delivers revisions and their events
  to a callback on a thread it owns. The installed CMake package now links
  `Threads::Threads` for the observer.
- Added bounded revision waits to Rust storage.
  `SqliteProduction::revision_waiter` returns a waiter with its own read
  connection that blocks until the first revisions after a sequence exist, for
  at most 60 seconds, and otherwise reports a timeout, a closed production, or
  cancellation from another thread. Commits through the same production wake it
  immediately; commits from other processes and handles are detected by polling
  the SQLite data version with bounded backoff.
- Added event-type-filtered revision pages to Rust storage.
  `changes_since_filtered` returns the revisions after a sequence that contain
  at least one event of the requested types, plus a through sequence that is
  the next cursor, so consumers skip unrelated revisions without reading them.
- Added SQLite schema 13, which keys every journaled event by event kind and
  revision sequence in a trigger-maintained table backfilled from the existing
  journal, so a filtered revision page reads rows proportional to the page.
- Added an opt-in Rust `ffmpeg` subprocess executor and CLI `job run` worker for
  named proxy and thumbnail profiles, with configurable executable and timeout,
  bounded diagnostics, claim heartbeats, temporary-output cleanup, atomic
  publication, fingerprinted output, provenance snapshots, copied activity
  parameters, and capability-safe behavior when `ffmpeg` is absent.
- Added Rust storage operations to request, list, read, claim, renew, release,
  complete, fail, and cancel durable jobs, with caller-supplied lease time,
  token-checked worker transitions, atomic output/provenance persistence, job
  metadata parameters, semantic lifecycle events, and non-mutating regeneration
  plans derived from producing activities.
- Added SQLite schema 9 foundations for durable jobs, canonical inputs,
  lifecycle detail, job metadata, and indexed state/kind queries.
- Added bounded, keyset-paginated dependency/dependent traversal and job
  queries across Rust, C, C++17, Python, and CLI, with opaque query-scoped
  cursors, explicit dependency traversal bounds, shortest-depth matches, and
  optional exact job state/kind filters.
- Added the Rust domain-query foundation for bounded asset, representation,
  resource, locator, media-root, unresolved-media, metadata, activity-output,
  provenance, stale-artifact, and changed-object pages. Producing and consuming
  activity selection now occurs in SQL, and representation pages use
  set-oriented structure and fingerprint loading.
- Added the bounded domain queries to C ABI version 25, the C++17 wrapper,
  Python, and the CLI: asset, representation, resource, and locator pages;
  representations under a logical media root; knowledge-only unresolved media;
  metadata-property pages with an optional exact scalar value; activities
  producing or consuming a representation; outputs by activity kind or exact
  tool identity; depth-bounded provenance ancestors and descendants; stale
  artifacts, optionally restricted to descendants of one source; and objects
  changed after a revision sequence. Locators can record the logical root they
  were confirmed under. The whole-set asset and representation enumerations of
  the 0.3.x integration subset remain available unchanged.
- Added SQLite schema 12 query-support tables for unresolved memberships,
  representations under a logical root, and activity-output kind and tool keys.
  Triggers maintain them on every write path, and migration backfills them from
  existing knowledge, so those pages read rows proportional to the page rather
  than to the production.
- Added a `release_0_4_queries` benchmark that measures one page of each domain
  query on the 10,000-asset fixture. The fixture generator now records logical
  roots, sparse unresolved media, snapshotted transcodes with a stale fraction,
  and journal events spread over every asset; delete a cached fixture from an
  earlier generator version before rerunning.
- Added SQLite schema 11 query indexes and optional durable logical-root
  knowledge on confirmed locators. Existing locators migrate without fabricated
  root knowledge.
- Added SQLite schema 10's `(kind, id)` job index for kind-only cursor queries.
- Added first-class, typed dependency observations with exact authored
  references, floating or pinned targets, complete-set replacement, semantic
  revision events, and SQLite schema 8 foundations for bounded activity-input
  dependency snapshots.
- Added SQLite schema 7 foundations for explicit, journaled fingerprint
  observations with retained history, representation recomputation markers,
  and storage-captured activity input/output snapshots. Existing activities
  migrate with absent snapshots rather than fabricated historical state.
- Added C ABI version 16 transaction functions for explicitly recording
  resource and structure-aware representation fingerprint observations, plus
  inspection of storage-captured activity-edge fingerprint snapshots.
- Added computed artifact knowledge states with bounded transitive staleness,
  divergence explanations, and structured reproducibility reports. C ABI
  version 17 exposes both read models through owned immutable result handles,
  with matching C++17, Python, and CLI surfaces.
- Added C ABI version 18 dependency-aware artifact explanations with typed,
  exact authored paths and explicit unresolved, dirty, and truncated states.

### Changed

- Ended the integration-preview subset with the 0.3.x series; every 0.4 API is
  experimental (ADR 0020).
- Image-sequence file names belong to each locator, so a renamed sequence is
  found by content and confirmed under its new naming, on every surface
  (ADR 0038, C ABI 35, schema 15). Version 1 sequence and representation
  fingerprints are no longer computed; observe a sequence again to compare it.
- Media sources describe a single file, image sequence, ordered parts, or
  package for both importing an asset's original and adding a representation,
  on every surface; they replace the four per-structure add functions
  (ADR 0037, C ABI 34).
- C++ metadata values are readable: `MetadataValue` replaces `MetadataInput`
  for writing and reading (ADR 0036).
- Observing content reports whether it was unchanged, changed, or observed for
  the first time, on every surface (ADR 0035, C ABI 33).
- The C++ wrapper returns `postproject::Result<T>` from every fallible
  operation and compiles without exceptions; `value()` throws
  `postproject::Exception` when they are enabled (ADR 0032).
- C++ `Result` uses the `std::expected` member names and combinators, and
  `POSTPROJECT_TRY` and `POSTPROJECT_TRY_ASSIGN` are public (ADR 0033).
- Resolution searches unnamed search directories as well as mapped roots,
  resolves many assets with one scan, budgets entries per directory instead of
  failing, can be cancelled, and names each candidate's root, on every surface
  (ADR 0031, C ABI 32).
- `pp_string_release` releases every owned string, replacing
  `pp_host_binding_release`.
- External-identifier lookup can require an exact qualifier on every surface
  (ADR 0002, C ABI 30).
- `postproject media fingerprint` takes only a resource and a path, and a new
  `postproject media verify-content` command compares content with it.
- Downstream integration runs now include the OpenAssetIO Manager suite.
- Bumped the pre-release C ABI to version 26 with event-kind-filtered revision
  pages and revision waiters: `pp_revision_waiter_create`,
  `pp_revision_waiter_wait`, a thread-safe `pp_revision_waiter_cancel`, and
  `pp_revision_waiter_release`. Releasing a production closes its waiters.
- Bumped the pre-release C ABI to version 25 with bounded domain-query pages,
  scalar metadata predicates, activity-output filters, explicit provenance
  bounds, stale-artifact and changed-object queries, and root-aware locator
  confirmation.
- Paginated the experimental CLI commands `activity producing`, `activity
  consuming`, `activity ancestors`, `activity descendants`, and `metadata find`.
  Their JSON output is now a page object with `items`, `next_cursor`, and
  `traversal_truncated` instead of a bare array; they accept `--limit` and
  `--cursor`, ancestors and descendants accept `--max-depth` and
  `--max-representations` and report each match's depth, and `metadata find`
  accepts `--value-file` for an exact scalar value. `media list` keeps its
  complete-list output unless `--limit` or `--cursor` is given.
- Bumped the pre-release C ABI to version 24. Complete dependent and job lists
  are replaced by bounded pages, and forward dependency traversal is public.
- Bumped the pre-release C ABI to version 23 with read-only regeneration plans
  containing the artifact, proposed job, and copied job parameters.
- Bumped the pre-release C ABI to version 22 with atomic job completion over a
  representation and activity staged in the same transaction.
- Bumped the pre-release C ABI to version 21 with transaction-staged job claim,
  lease renewal, release, failure, and administrative cancellation.
- Bumped the pre-release C ABI to version 20 with durable job listing and
  transaction-staged job requests.
- Bumped the pre-release C ABI to version 19 for the job object-reference kind
  and job lifecycle revision events across C, C++, Python, and CLI projections.
- Moved the repository and its integration repositories to the
  `postproject-org` GitHub organization; `eseifert/postproject` URLs redirect.

### Fixed

- The Python binding loads and checks each native library once per path
  instead of on every call of a module-level function.
- An image sequence whose recorded directory no longer holds any of its frames
  is searched for like other moved media instead of resolving as an error.
- Regeneration plans repeat the kind and target root of the job that produced
  the artifact, found through a new SQLite schema 14 index (ADR 0023).
- The C++ header no longer raises `-Wmaybe-uninitialized` in `evaluateArtifact`
  under GCC 15, and CI compiles it with the current GCC and Clang (ADR 0034).
- Observing content records the resource's size and modification time too, so
  resolution still finds content whose size changed.
- Resolution no longer ignores resources whose only fingerprints are foreign to
  PostProject; it reports them as not verified on every surface (ADR 0028,
  C ABI 28).
- `postproject revisions events` now prints locator-retirement, media-root
  enablement, and media-root removal events instead of failing on them.

## 0.3.0-alpha.2 - 2026-09-24

### Changed

- Added a site-wide code-language selector to the documentation. Guide examples
  now appear as synchronized C, C++, Python, Rust, and CLI tabs, with an explicit
  note where a surface lacks an operation, and every example is extracted from
  programs that CI compiles and runs against the installed package. New guides
  cover creating a production, external identifiers, and media roots and
  resolution; the other integrator guides are now language-neutral.
- Published the complete 0.3.0-alpha.2 documentation under both its immutable
  release path and `latest`, and marked alpha GitHub releases as prereleases.

## 0.3.0-alpha.1 - 2026-09-23

### Changed

- Matched the documentation theme to the project landing page's fonts and
  colors and added a link back to it.
- Replaced the mdBook-only documentation build with one versioned Sphinx site
  combining the guides, generated Doxygen C/C++ reference, Python autodoc, and
  links to matching rustdoc.
- Advanced the development version to `0.3.0-alpha.1` and introduced a named
  integration-preview subset that remains compatible within the 0.3.x series.
- Replaced absolute media-root identity with unique logical names in SQLite
  schema 6. Migrated roots retain their former URI as a lossless legacy fallback.
- Advanced the C ABI to version 15 for portable root summaries, per-call
  machine-root mappings, and unmapped/unavailable resolution evidence.
- Raised the Python binding's minimum supported version to Python 3.11.
- Renamed the durable root container from `Project` to `Production` across the
  domain, SQLite schema, CLI, C ABI version 8, C++ wrapper, and Python binding;
  `.pproj` remains the PostProject storage-format extension.
- Renamed the project and repository identity to PostProject and
  `eseifert/postproject`; native Unix library files retain the conventional
  `libpostproject` name.
- Advanced the development version to `0.2.0-alpha.1` for the standards-aware
  domain-model work. Pre-1.0 API, ABI, schema, CLI, and binding compatibility is
  not promised.
- Replaced the flat native media-resolution result with ABI version 5's nested
  representation availability, resource results, and availability issues.
- Added ABI version 6 activity creation, inspection, direct provenance queries,
  and ancestry/descendant traversal with C++17 value wrappers.
- Added ABI version 9 representation structure, ordered membership, compact
  image-sequence, concrete resource, locator, and typed fingerprint inspection.
- Added ABI version 10 host-binding formatting and parsing, with C++ and Python
  wrappers over the same public C implementation.
- Added ABI version 11 owned typed-metadata inputs, including recursive lists
  and structures, and removed the superseded text-only write function.
- Added ABI version 12 creation of single-file, image-sequence, ordered-parts,
  and package representations, with matching C++, Python, and CLI surfaces.
- Added ABI version 13 asset enumeration with immutable summaries in C, C++,
  and Python.
- Added ABI version 14 media-root enumeration, root enable/disable/removal,
  locator retirement, and their semantic revision events across C, C++, Python,
  and the CLI.
- Added a strict, versioned HTTPS host-object binding format under
  `postproject.org` for portable production-scoped references.
- Added an opt-in registry for UMID, ISAN, EIDR, and application identifier
  schemes with local syntax checks and no network behavior; PostProject-owned
  application identifiers use `https://postproject.org/id/application`.
- Added an opt-in metadata vocabulary registry with type, cardinality,
  description, validation, and cross-standard mapping hints; unknown terms
  remain independent of the registry.
- Extended external-identifier attachments to activities in SQLite schema
  version 4, including exact lookup and transactional persistence.
- Added deterministic, structure-aware representation fingerprint computation
  for single resources, image sequences, ordered parts, and packages.
- Added tagged JSON input for every recursively typed metadata value in the
  CLI, while retaining the concise text command.

### Added

- Opt-in fingerprint verification for known file and sequence locators,
  explicit mismatch evidence, and conservative relative-path and technical
  profile candidate scoring exposed through `media resolve --verify`.
- A bounded optional `ffprobe` subprocess adapter that records normalized,
  vocabulary-backed technical metadata during CLI import while treating a
  missing inspector as a non-fatal capability gap.
- Adapter-level recognition for numbered image sequences, recording spans,
  metadata sidecars, and AVCHD camera-card packages, plus relocated-sequence
  resolution and compound original import from the CLI.
- Read-only media inventory with machine-inspectable categories, bounded root
  traversal, a disposable versioned sidecar cache, and CLI JSON output.
- Machine-local mappings from portable root names to local directories, with
  explicit unmapped/unavailable evidence and continued scanning of usable roots.
- A tag-triggered GitHub release workflow publishing checksummed source,
  Linux, macOS, and Windows native archives plus a tested Python wheel. Native
  archives include the CLI so installed workflows do not require Cargo.
- A stewardship policy covering governance, compatibility, security reports,
  releases, and the transition to broader maintainership.
- Installed, cross-platform-tested C, C++, and Python quickstarts with a small
  deterministic media fixture.
- An mdBook documentation foundation split across application-user,
  integrator, contributor, concept, and reference sections.
- Accepted architecture decisions for naming, compound media, identity,
  metadata, provenance, rational time, revision events, bindings, persistence,
  OpenAssetIO boundaries, namespaces, concurrency, and availability.
- Strong production, asset, representation, resource, locator, activity,
  revision, media-root, and transaction IDs plus typed cross-object references.
- A compound-media model for single resources, compact image sequences,
  ordered parts, and packages with required or optional resource roles.
- Canonical SQLite schema version 6 with transactional compound media,
  external identifiers, typed metadata, provenance, lifecycle operations, and
  a durable semantic revision journal.
- C ABI version 15 and C++17 wrappers for the complete public model, including
  compound-media creation and inspection, provenance, revisions, host bindings,
  asset/root enumeration, and locator/root lifecycle operations.
- A Python 3.11 binding over the public C ABI with generated signatures,
  compiler-verified layouts, structured errors, and deterministic cleanup.
- Activity-based provenance with extensible kinds and edge roles, bounded tool
  and agent identity, cycle prevention, deterministic reload, and graph queries.
- Structured, repeated, language-tagged metadata with deterministic typed
  persistence and bounded recursive values.
- Extensible external identifiers on assets, representations, resources, and
  activities, including exact scheme/value lookup and opaque round-trips.
- Exact rational-time comparison, lossless checked rescaling, half-open ranges,
  and deterministic text round-trips for common integer and fractional rates.
- A durable semantic revision feed with ordered events committed atomically
  alongside domain mutations.
- Deterministic resource and representation fingerprints, including sampled
  content evidence for compact image sequences.
- Structured five-state representation availability with resource/member
  diagnostics and explicit ambiguity.
- CLI commands for compound media, identifiers, typed metadata, provenance,
  revisions, resolution, and explicit confirmation with structured JSON output.
- Initial workspace and engineering-policy scaffolding.
- Dual MIT or Apache-2.0 licensing.
- Automated advisory, source, duplicate-dependency, and license policy checks.
- Installable CMake and `pkg-config` metadata with a standalone native consumer test.
- Criterion baselines for bulk import, large-production open, resolver scans, and commits.
- End-to-end relocation coverage for single-file, sequence, and ordered media,
  including partial availability and preserved provenance.
- `cargo-fuzz` targets for production files, fingerprints, identifiers, compound
  structures, metadata, revisions, rational time, C strings, and strong IDs.
- ASan/UBSan native-consumer CI and a standalone installed-package C example.
- Shared/static native artifacts, installed release documentation, and release checklist.
- Bounded SQLite value and row sizes when opening untrusted production files.
- Unambiguous cross-platform CMake metadata with explicit Windows DLL packaging.
- Backend-neutral, domain-shaped production read and transaction contracts.
- Release 0.1 acceptance report with verification and benchmark summaries.
- Locked the compatible `yoke-derive` patch release to preserve Rust 1.85 support.
