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

## Explicitly later

- full IPTC VMH and EBUCorePlus mapping packages;
- W3C PROV and MovieLabs OMC import/export adapters;
- C2PA assertion, signing, and verification integration;
- MXF, AAF, and IMF adapters;
- OTIO adapters beyond a possible media-linker proof;
- collections, asset groups, and package models;
- persistent filesystem indexing and background job execution;
- PostgreSQL, a network daemon, collaboration, locking, and conflict handling;
- timeline/editorial models, GObject/Qt adapters, extraction, transcription, and
  semantic search;
- remote object storage, undo/redo, and distributed revision merging.
