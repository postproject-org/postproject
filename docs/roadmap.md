# Roadmap

## Release 0.1 — complete

Deliver stable media identity, SQLite persistence, versioned fingerprints,
deterministic relinking, a C ABI, C++ wrapper, CLI, packaging, and comprehensive
tests. The delivered scope and remaining constraints are recorded in the
[acceptance report](release-0.1-report.md).

## Release 0.2 — complete

Establish standards-aware internal/external identity, structured and repeated
metadata, activity-based provenance, rational time, a durable semantic revision
journal, and a Python binding over the public C ABI. Documentation is organized
for application users, integrators, and contributors. The delivered scope and
remaining constraints are recorded in the
[acceptance report](release-0.2-report.md).

## Release 0.3 — complete

Add portable named storage roots, non-mutating inventory scans, compound-media
recognition, optional technical inspection, opt-in content verification,
OpenAssetIO and OTIO integration demonstrations, and a published unified API
site. The delivered scope and remaining constraints are recorded in the
[acceptance report](release-0.3-report.md).

## Release 0.4 — complete

Add managed-artifact knowledge, dependency-aware staleness, durable production
jobs, scalable domain queries, cross-process change delivery, publishing, and a
maintained application pilot. Fingerprint observations and activity snapshots,
artifact evaluation and reproducibility, dependency relationships, and the
complete host-worker job protocol are delivered on Rust, C, C++, Python, and
the CLI. An opt-in `ffmpeg` reference executor is delivered through Rust and
the CLI with named proxy and thumbnail profiles, lease heartbeats, atomic job
completion, and bounded failure cleanup. PostProject still never starts work
implicitly. Bounded domain queries and change delivery — event-kind-filtered
revision pages and a cross-process revision wait with C++ and Python observers —
are delivered on every surface. The maintained OpenAssetIO Manager publishes
rendered media through the job protocol. The
[Kdenlive pilot](https://github.com/postproject-org/postproject-kdenlive), a
patch series on Kdenlive `v26.08.1`, relinks renamed clips by content through a
sidecar production. It records the proxies Kdenlive renders as managed
artifacts through the job protocol, and rebuilds a proxy whose source was
replaced. A tested Flatpak module builds PostProject for it on the
KDE SDK. The
[Blender pilot](https://github.com/postproject-org/postproject-blender), an
extension for Blender 5.2 and 5.3, relinks renamed movies, sounds, and image
sequences by content through a sidecar or Blender project production. It led
to media sources for importing any content structure, sequence names on
locators so renamed sequences are found, and platform wheels that carry the
native library. What the pilots taught is recorded in the
[integration findings](release-0.4-integration-findings.md), and the delivered
scope and remaining constraints in the
[acceptance report](release-0.4-report.md).

## Release 0.5 — shared local productions

Make one local production useful to multiple host processes. Exact locator and
fingerprint lookup lets a host explicitly adopt media already recorded by
another host. Optional transaction base revisions and typed semantic conflicts
prevent silent overwrites while independent additive facts continue to merge.
The public C ABI, C++ wrapper, Python binding, Rust API, and CLI expose the same
conflict key and superseding-revision information.

The maintained Kdenlive and Blender paths now use one explicitly selected
production in an automated scenario: Blender adopts Kdenlive's asset, records a
derived render, observes a Kdenlive locator change, and wins a deliberate
same-base locator conflict whose structured result reaches Kdenlive. The
OpenAssetIO Manager resolves the render, and replacing the source makes both
hosts' derived outputs stale. The evidence review selected only an
[Ardour resolver pilot](release-0.5-ardour-pilot.md), which now exercises the
installed `pkg-config` package, exception-enabled C++ use, and realistic audio
recovery without assigning job semantics to waveform caches. The accepted
compatibility review names no 0.5 subset because the mechanically eligible
leaf families lack qualified lifecycle dependencies. The delivered scope and
verification evidence are recorded in the
[acceptance report](release-0.5-report.md).
Shared access remains one local SQLite file on one machine; services, network
filesystems, permissions, and distributed merging are outside this release.

## Release 0.6 — native integration candidate

Add a direct-C OBS finalized-recording plugin and a native C++ Natron Reader
adapter against installed public packages. OBS records capture knowledge off
its processing callbacks; Blender adopts and resolves that recording through
its maintained integration. Natron explicitly adopts compact sequences,
preserves its filename fallback, and handles missing frames, ambiguity,
obsolete asynchronous results and an independent writer's conflict.

A closed-transaction destruction defect is repaired. Compatibility evidence
checks complete projections and dependency closure across released 0.5 and
the 0.6 candidate. The approved C++ Result propagation family remains
source-compatible within `0.6.x`. Audacity and Krita were
not selected under the documented minimum-pilot gate. The
[acceptance report](release-0.6-report.md) records exact verification and
release-scoped limitations. Package release tagging and publication remain
outside this candidate handoff.

## Explicitly later

- full IPTC VMH and EBUCorePlus mapping packages;
- W3C PROV and MovieLabs OMC import/export adapters;
- C2PA assertion, signing, and verification integration;
- MXF, AAF, and IMF adapters;
- OTIO adapters beyond a possible media-linker proof;
- collections, asset groups, and package models;
- persistent filesystem indexing and background job execution;
- PostgreSQL, a network daemon, distributed collaboration, and locking;
- timeline/editorial models, GObject/Qt adapters, extraction, transcription, and
  semantic search;
- remote object storage, undo/redo, and distributed revision merging.
