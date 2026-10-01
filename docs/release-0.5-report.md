# Release 0.5 acceptance report

Release 0.5 makes one local production useful to several application
processes. A host can adopt media another host already recorded, observe facts
written by that host, and protect read–decide–write operations with typed
semantic conflicts. The delivered release is package `0.5.0-alpha.1`, C ABI 37
with 230 exported symbols, and SQLite schema 17. Architecture decisions 0040
through 0043 are accepted, and ADR 0020 records the release-series
compatibility decision.

## Delivered scope

- Bounded known-media lookup maps an exact current locator identity or current
  effective resource fingerprint back to its resource, representation, and
  asset. Lookup returns every candidate and never adopts, merges, or mutates.
  Hosts explicitly attach their own qualified identifiers after choosing an
  asset. Schema 16 indexes both lookup paths.
- A transaction may name the durable revision on which its decisions were
  based. Non-mergeable writes are classified by semantic conflict key and
  checked against derived conflict versions at commit. A conflict rolls back
  every staged change and reports the key, affected object, base revision, and
  superseding revision as structured data on Rust, C, C++, Python, and CLI
  surfaces. Schema 17 keeps conflict checks proportional to touched keys rather
  than revision-history length.
- Every revision-event kind has an explicit `merge`, semantic-conflict,
  existing-guard, or not-applicable classification. Existing job claim tokens
  remain authoritative for worker concurrency.
- Shared production means one explicitly selected local SQLite `.pproj` file
  opened by several processes on one machine. Hosts identify their commits,
  discover durable changes through revision cursors, and never read another
  host's project format. Network filesystems and service/database deployments
  are outside this contract.
- An opt-in C-ABI boundary trace records sorted, deduplicated operation names
  without arguments or production data. A machine-readable family manifest and
  deterministic report turn maintained-host traces into all-or-nothing
  compatibility evidence.
- Interactive latency targets and Criterion coverage now include known-media
  lookup, concurrent writers, and 100-key semantic-conflict checks. The targets
  remain informational; 0.5 defines no timing-based CI release gate.
- The cross-platform core acceptance test drives compound-media import, a
  successful and failed fake-FFmpeg worker, cross-process revision delivery,
  provenance snapshots, staleness, regeneration planning, and dependency
  propagation through the public CLI.

## Compatibility

Release 0.5 names no compatibility subset. Every API in the `0.5.x` series
remains experimental and may change within the series, with migration notes for
operations used by maintained integrations.

The generated {doc}`release-0.5-compatibility-evidence` matrix found external
identifiers and resolution mechanically eligible across the 0.4-to-0.5
boundary. The accepted
{doc}`release-0.5-compatibility-decision` does not promise those leaf families
in isolation: both require the production lifecycle, and identifier mutation
also requires the transaction lifecycle, while those foundational families
lack complete evidence from two independent hosts.

Productions from every published schema migrate forward to schema 17. Schema
16 adds only derived lookup indexes. Schema 17 adds derived conflict versions
and a conservative migration baseline; it does not discard existing production
knowledge. This forward-migration policy is not a general ABI or schema
compatibility promise.

## Integration validation

On 2026-10-01, the maintained Kdenlive and Blender paths plus the OpenAssetIO
Manager completed the automated Linux
{doc}`release-0.5-integration-findings` scenario against one explicit
production:

- Blender adopted Kdenlive's camera asset rather than importing a duplicate.
- Blender recorded a derived render with provenance and Kdenlive observed it
  without opening the Blender file.
- The Manager resolved that render through OpenAssetIO.
- A confirmed moved-source locator reached the other host through revisions.
- Competing locator decisions from one base produced one commit and one typed
  conflict with no partial loser write.
- Replacing the source made both the Kdenlive proxy and Blender render stale.

The maintained integration suites generated the ABI traces used by the family
matrix. Downstream package bounds include the 0.5 series; the OpenAssetIO and
OTIO validation repositories, Manager, OTIO demonstration, C++ NLE, and Python
host passed locally against this tree.

The selected {doc}`release-0.5-ardour-pilot` adds audio-domain evidence for the
installed `postproject.pc` package and exception-enabled C++. Its executable
resolver scenario covers exact renamed audio, ambiguity, verified known
locations, external identity/origin, and typed exceptions. The adapter
translation unit compiles against the pinned Ardour headers and installed
PostProject package.

Remote CI on 2026-10-01 passed the
[Kdenlive builds and shared-production scenario](https://github.com/postproject-org/postproject-kdenlive/actions/runs/36847303316)
and the [Ardour builds and resolver scenarios](https://github.com/postproject-org/postproject-ardour/actions/runs/36840953154).
Ardour compiled with and without PostProject on Linux, and its installed-package
resolver passed on Linux and macOS. These runs used PostProject `934b0fe`.

## Verification summary

The release candidate passes locally:

- workspace formatting, Clippy with warnings denied, all-feature tests,
  warning-free rustdoc, and both Cargo dependency policies;
- the separately locked fuzz package's formatting, Clippy audit, and dependency
  policy;
- all 230 expected ABI symbols, the C smoke scenario, and the installed C and
  C++ consumers, including C++ without exceptions;
- package installation into an isolated prefix with shared and static
  libraries, CLI, headers, CMake metadata, `pkg-config` metadata, examples,
  licenses, changelog, and stewardship policy;
- 56 installed-package documentation scenarios across C, C++, Python, and CLI;
- 44 Python binding tests against the optimized native library;
- a fresh neutral wheel, derived Linux platform wheel, isolated installation,
  and quickstart without a library-path override;
- version consistency, Flatpak metadata checks, generated ABI declarations and
  layouts, example coverage, Ruff, and Python type checks;
- Doxygen coverage and normalization plus the complete Sphinx site with
  warnings treated as errors; and
- the Criterion quick suite, including the with-base and without-base
  100-locator-key commit paths. Its measurements are informational and are not
  compared to a release threshold.

Repository CI runs the workspace and core acceptance test on Linux, macOS, and
Windows and defines native packages and wheels for all three systems. The tag
workflow produces checksummed source, native, and Python artifacts; tagging and
publication remain maintainer-operated.

## Known constraints

- Shared production is limited to processes on one machine using a local
  SQLite file. Network filesystems, remote streams, server databases, access
  control, and distributed merge are unsupported.
- The automated background Kdenlive/Blender scenario passed, but no interactive
  graphical pass was performed in this environment. The pilots surface
  structured results in adapter coverage rather than polished cross-host
  conflict or revision UI.
- Blender records a completed render after the fact because its extension path
  has no reliable render-failure terminal. It does not pretend to be a job
  worker or a complete reproducible recipe.
- The full pinned Ardour application was not configured in this Gentoo
  workspace because its system dependency set lacks `liblo`. The resolver
  executable and actual adapter translation unit were compiled separately.
  Full Linux builds and the macOS resolver scenario passed remotely;
  interactive Ardour recovery was not exercised.
- No maintained application uses the raw C ABI directly. Installed consumers
  and examples validate its ownership, lifetime, and error mechanics.
- No second host exercises job renew, fail, and cancel together. Those
  operations are not part of a 0.5 compatibility promise.
- Performance targets are engineering guidance, not hard gates. Release 0.5
  remains pre-1.0 and names no API compatibility subset.
