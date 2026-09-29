# Release 0.4 acceptance report

Release 0.4 lets applications build repeatable production workflows on
PostProject: generated media is recorded with what it was made from, work is
requested and completed through durable jobs, and changes reach other processes
without rescanning. The delivered release is package `0.4.0-alpha.1`, C ABI 35
with 222 exported symbols, and SQLite schema 15. Architecture decisions 0021
to 0039 are accepted.

## Delivered scope

- Fingerprints are observations: content can be observed again, each change is
  journaled, and an activity records a snapshot of each input when it is
  committed. A host can supply its own content hash as a foreign fingerprint
  and have PostProject compute and observe it.
- Artifact evaluation reports whether generated media is current, stale, or
  indeterminate, naming the changed input, and a reproducibility report says
  whether its activity can be repeated. Evaluation never changes a production.
- Dependency relationships record that one object needs another each time it
  is used. A render is stale when a layer its input references changes, and
  both directions can be queried.
- Jobs persist requested work in the production. Any worker claims a job under
  a lease, renews it, and completes it atomically with its output, activity,
  and snapshots, or fails, releases, or cancels it. A regeneration plan
  proposes the job that would make a stale artifact current without enqueuing
  it.
- An opt-in reference executor runs `ffmpeg` as a subprocess to render proxies
  and thumbnails through the public job protocol, from Rust and the CLI.
- Named domain queries return bounded pages with keyset cursors: unresolved
  media, representations under a root, metadata properties, outputs by
  activity, provenance traversal, stale artifacts, dependencies, jobs, and
  objects changed since a revision. Point reads return one asset,
  representation, or job by identity.
- Revision pages can be filtered by event kind, and a revision waiter blocks
  for bounded time until another thread or process commits. C++ and Python
  observers deliver revisions on threads they own; no callback crosses the
  C ABI.
- Resolution searches unnamed search directories as well as mapped roots,
  resolves many assets with one scan, budgets its work, can be cancelled, and
  names each candidate's root. Image-sequence file names belong to each
  locator, so a renamed sequence is found by content.
- Media sources describe a single file, image sequence, ordered parts, or
  package for importing an asset and for adding a representation.
- The C++ wrapper returns `postproject::Result` from every fallible operation
  and compiles without exceptions and without warnings under current GCC and
  Clang. Python platform wheels carry the native library, and the Linux
  package and wheel load on glibc 2.28 or newer.

## Compatibility

Release 0.4 makes no compatibility promise. The integration-preview subset of
ADR 0020 ended with the `0.3.x` series, and every API of the `0.4.x` series is
experimental. Resolution, structure inspection, locator confirmation, and
host-object reference release changed from `0.3.x`; the changelog's migration
notes describe each change. Productions from every 0.3 release migrate to
schema 15 when opened.

## Integration validation

Two maintained application pilots use PostProject from outside this
repository, and their findings are recorded in the
[integration findings](release-0.4-integration-findings.md):

- The [Kdenlive pilot](https://github.com/postproject-org/postproject-kdenlive),
  a patch series on Kdenlive `v26.08.1` that links only the installed CMake
  package, relinks a renamed clip by content through a sidecar production and
  records the proxies Kdenlive renders as jobs, rebuilding a proxy whose source
  was replaced. Kdenlive's own tests cover both workflows, and a Flatpak module
  builds PostProject on the KDE 6.10 SDK.
- The [Blender pilot](https://github.com/postproject-org/postproject-blender),
  an extension for Blender 5.2 and 5.3 that uses only the Python binding,
  relinks renamed movies, sounds, and image sequences by content.

The maintained
[OpenAssetIO Manager](https://github.com/postproject-org/postproject-openassetio-manager)
0.2.0 reads entities by identity and publishes rendered media through the job
protocol, including an EXR sequence as one representation observed from a
second process. The
[OTIO demonstration](https://github.com/postproject-org/postproject-otio-demo)
depends on this release series and the Manager. PostProject CI runs the Manager
suite and the four validation repositories against every revision, and both
pilots run nightly against `main`.

## Verification summary

The release candidate passes:

- workspace formatting, Clippy with warnings denied, all-feature tests, and
  warning-free rustdoc, compiled on Rust 1.85 exactly;
- both Cargo dependency policies and the separately locked fuzz package;
- all 222 exported ABI symbols, the C smoke test, native address and
  undefined-behavior sanitizers, installed C and C++ consumers, and the C++
  header with the current GCC and Clang;
- the Python binding tests against the release library, the platform wheels
  from a fresh virtual environment, and the Linux wheel on glibc 2.28;
- tested documentation examples for every C function, C++ member function,
  Python method, and CLI command, extracted into the guides;
- the reference executor with real FFmpeg on Linux and fake executables on
  Linux, macOS, and Windows;
- Doxygen symbol coverage, the complete Sphinx build with warnings treated as
  errors, and link checking;
- the Manager, OTIO demonstration, validation repositories, both pilots, and
  the Flatpak module against the release candidate.

Benchmarks on a 10,000-asset fixture with 100,000 representations are recorded
in [benchmarks](benchmarks.md). Each domain-query page reads rows in proportion
to the page, not to the production.

## Published surfaces

The tag-triggered release publishes checksummed source, native Linux, macOS,
and Windows archives, a Python platform wheel for each, the platform-neutral
Python wheel, and the Flatpak module with its Cargo sources. The Pages workflow
publishes the development documentation and immutable documentation built from
the release tag.

## Known constraints

- The benchmark numbers are informational. Release 0.4 records no performance
  budgets, and loading every asset with its identity evidence on the
  10,000-asset fixture takes about one second.
- The release workflow is verified surface by surface and through the pilots
  and the Manager; the repository has no single end-to-end acceptance test for
  it.
- The reference executor is reachable from Rust and the CLI only. C, C++, and
  Python hosts act as workers through the job protocol themselves.
- Artifact evaluation reports what the production knows. Availability and
  pending work are separate reads, not one combined state.
- Media published through OpenAssetIO records no inputs, because OpenAssetIO
  carries none.
- Releases are checksummed but not code-signed. PostProject remains pre-1.0,
  and no API is promised to stay compatible within the 0.4 series.
