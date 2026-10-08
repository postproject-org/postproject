# PostProject 0.7 candidate acceptance

Candidate `0.7.0-alpha.1` (Python `0.7.0a1`), C ABI **51**, **322 exported
symbols**, SQLite schema **19**. The released baseline is `v0.6.0-alpha.1`,
commit `a413227b9dd048f744fa7393d8927891ee93d5e2`.
The new minor series accommodates changed experimental signatures and domain
contracts. Schema 18 journals file facts; schema 19 adds authority-timed leases
and expires legacy claims without losing requests or attribution.

## Delivered behavior

- Native semantic IDs, checked references and content/job/artifact/resolution
  alternatives; authoritative constructor and decode validation.
- Real coherent reads, automatically scoped decision bases, explicit edits and
  atomic receipts. Commit attempts are terminal; no-op commits add no revision.
- Production-bound worker leases, exact checked durations, authority-controlled
  time, private capability transport and atomic guarded output/provenance.
- Bounded reads, cursors, collection payloads and resolution work; cancellation,
  structured conflicts and explicit retry decisions across public surfaces.
- Python UUID hints over ordinary UUIDs, Fraction rates, timedelta leases and
  explicit transaction commits; CLI format 1 with exact receipts/error codes.

The {doc}`api-safety-audit` maps public families to resolved decisions and
semantic regressions. ADRs 0045–0064 record contract and standards-impact
decisions. No daemon, distributed protocol, replica or new standards mapping
is delivered.

## Verification

Local Linux checks pass: the six required Rust gates, exact Rust 1.85 floor,
Rust 1.99 Clippy, 322 exports and 27 Rust/C/ctypes layouts, nine installed native
contracts, C/C++ ASan/UBSan, 121 Python tests (one skip), installed typing
positives/nine negatives, 72 extracted examples, strict docs/links/spelling,
five sanitized fuzz campaigns and all six scale/benchmark workloads.
Benchmarks are informational; no hard latency claim applies.

All maintained consumers use the installed candidate. The
{doc}`api-safety-repositories` records commits, artifacts, checksums, commands
and scope. Platform/package and full-host CI qualification is in progress;
release readiness is not yet claimed.

Both required handoffs pass locally. OBS 32.2.2 records a finalized output;
Blender 5.2.2 adopts the original asset, preserves capture provenance, resolves
a move and reopens without the extension. Kdenlive 26.08.1, Blender and the
Manager exercise shared adoption, render provenance, moved locators,
competing decisions, structured conflict, refresh and staleness.
Natron 2.5.0 passes normal/negative/plugin-free renderer paths and generation
checks; Ardour 9.8 passes its installed audio resolver scenario.

These are automated host tests. No interactive human pass, upstream
endorsement or full Natron source rebuild is claimed. Blender's optional
5.3-alpha CI cannot download a daily build; its required 5.2.2 suite passes.
The second real renew/fail/cancel consumer remains absent from compatibility
evidence, rather than being replaced by contract-test counts.

## Compatibility and publication

The {doc}`release-0.7-compatibility-evidence` distinguishes current execution
from historical eligibility. Changed ABI families do not inherit unchanged
status. The released 0.6.x C++ Result propagation promise and published
artifacts remain intact. No additional stable family, 1.0 contract or
cross-series binary replacement is named.

SDK and consumer changes are on GitHub `main`; the landing site retains
published 0.6 links. Core artifacts precede consumer package publication;
Manager 0.4 precedes the demo that requires it. Matching candidate artifacts
already qualify consumers without a circular release dependency.

No release tag or package publication has occurred. Tagging and publication
require explicit authorization and the checks in {doc}`releasing`; published
assets must be verified against their signed tag and checksum manifest.
