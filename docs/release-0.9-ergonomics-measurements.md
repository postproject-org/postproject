# Python API measurements for 0.9

The published 0.8.0a1 wheel and installed 0.9.0a1/ABI 53 workstation wheel read
the same schema-27 fixture: 5,000 assets and 1,050 revisions. Three fresh-process
samples per successful path use Python 3.11.15 and `tracemalloc` on Linux.
[Inputs](evidence/release-0.9/ergonomics-measurement-inputs.json) and
[raw samples](evidence/release-0.9/ergonomics-measurements.tsv) preserve provenance.
The [schema-27 storage/history measurements](schema-27-exchange-measurements.md)
remain the separate 0.8 baseline; this source redesign changes no format.

| Path | Published 0.8 median | 0.9 median | 0.9 Python allocation peak |
|---|---:|---:|---:|
| All assets, pages/scan of 100 | 1.007 s, manual pages | 1.029 s, scan | 780 KB |
| All revisions, pages/scan of 100 | 0.788 s, manual pages | 0.799 s, scan | 794 KB |
| Stage 1,000 metadata values | 0.764 s, explicit types | 0.762 s, native pairs/integers | 719 KB |
| Resolve one asset 100 times | 0.810 s | 0.770 s | 774 KB |

Asset scan pages of 10, 100 and 1,000 peak at 733 KB, 780 KB and 1,244 KB
respectively. The page size controls retained copied values; collecting the
whole scan into a list deliberately forfeits that bound. The old eager asset
collection rejects this fixture with its documented convenience-bound error.
It supplies no comparable full-scan timing. Its manual pages still complete.

The workload-specific regression allowance is twice the corresponding 0.8
median for equivalent complete work. This leaves room for interpreter/trace
overhead while detecting gross extra scans or per-value setup costs; all four
paths pass. These measurements include open/validation and actual read/staging
work, not isolated conversion microbenchmarks. Metadata staging rolls back.

Python allocation peaks exclude Rust/SQLite allocations. Raw process RSS also
includes interpreter and loader costs and is not interchangeable with those
peaks. The fixture establishes bounded behavior and comparable local costs;
it does not establish a general speedup, production-media search latency or
cross-platform performance.

## Qualified 0.9 and revised ABI 57

The same schema-27 fixture was read with the preserved ABI-53 wheel and the
ABI-57 wheel. Three fresh Python 3.11.15 processes per path use the same
workstation while a host build runs. {download}`Inputs and hashes
<evidence/release-0.9/abi57-measurements/inputs.json>` and individual samples
under `docs/evidence/release-0.9/abi57-measurements/` identify this scope.

| Complete path | ABI 53 median | ABI 57 median | ABI 57 Python allocation peak |
|---|---:|---:|---:|
| Scan 5,000 assets, pages of 100 | 0.994 s | 0.972 s | 775 KB |
| Scan 1,050 revisions, pages of 100 | 0.822 s | 0.815 s | 794 KB |
| Stage 1,000 native metadata values | 0.753 s | 0.785 s | 715 KB |
| Resolve one asset 100 times | 0.789 s | 1.036 s | 771 KB |

All paths remain within twice the corresponding qualified 0.9 median, the
same gross-regression allowance used above. Resolution now includes copied
verified-selection bookkeeping; this measurement establishes an acceptable
local cost, not a speedup. Metadata staging rolls back. The new conversion
captures at most 4,096 members and rejects depth beyond 32 before staging;
selection indexes required resources and adds no filesystem scan. Existing
page bounds and the Python-allocation/RSS distinction remain unchanged.
