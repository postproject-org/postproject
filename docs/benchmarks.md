# Benchmarks

Release 0.1 benchmarks are informational baselines built with Criterion. They
cover the four scale-sensitive workflows named in the acceptance criteria:

- fingerprinting and importing 1,000 small files in one transaction;
- opening and enumerating a SQLite production containing 10,000 assets with
  resource and representation fingerprints;
- resolving relocated media beneath a root containing 3,000 decoys;
- committing repeated single-import transactions.

Run them with an optimized build:

```sh
cargo bench --locked -p postproject-storage-sqlite --bench release_0_1
```

Fixture construction that is not part of the operation under measurement is
performed before timing where practical. The bulk-import benchmark deliberately
includes fingerprint preparation because fingerprinting is part of the import
contract. Resolver decoys have the same size as the target, so all 3,000
candidates reach fingerprint verification instead of being discarded by the
cheap size filter. The large-production workload traverses representations,
resources, and their non-empty fingerprint evidence rather than timing only the
root production row.

Criterion reports are local artifacts under `target/criterion` and are not
committed. Published numbers must record the commit, Rust version, operating
system, CPU, storage device/filesystem, power policy, and full Criterion command.
Results are not release gates yet; they exist to make regressions measurable.

## 0.5 interactive latency targets

The following are engineering targets for local productions on a supported
desktop, warm filesystem cache, release build, and the representative
10,000-asset fixture. They describe the latency a host should normally be able
to keep off its user's critical path; they are not CI or release gates.

| Interactive operation | Target |
| --- | ---: |
| Open a production | 50 ms |
| First page of a bounded structural, metadata, provenance, job, or revision query | 50 ms |
| First page of the whole-production stale-artifact query | 250 ms |
| Evaluate one artifact's staleness | 25 ms |
| Read one filtered revision page | 50 ms |
| Wake a waiter after another local process commits | 250 ms |
| Find known media by current locator or fingerprint | 25 ms |
| Check 100 touched optimistic-conflict keys during commit | 10 ms added commit time |

The waiter target begins when SQLite commits and ends when the waiting process
receives a non-empty revision page. It is not a transaction-duration target.
Lookup excludes hashing files: it measures lookup of already recorded locator
or fingerprint evidence. Conflict checking excludes the domain writes and
journal append that the transaction would perform without a base revision.

Measurements must report the fixture generator version, page size, result
count, cache state, commit, Rust version, operating system, CPU, storage, and
power policy. New operations first receive a benchmark and several CI/local
observations. A target becomes a hard regression limit only through a separate
documented change with evidence that normal runner variance will not produce
false failures.

There are currently **no hard performance limits in CI**. Functional bounds
such as maximum page size, traversal depth, and result count are correctness
limits, not performance thresholds. CI benchmark smoke runs may fail when an
operation is wrong or unbounded, but they do not compare elapsed time to the
targets above.

## Release 0.4 scale fixture

The `large_fixture` benchmark target is a deterministic generator rather than
a timed benchmark. It creates the representative production used by the
0.4 query, staleness, job, and revision-feed benchmarks:

- 10,000 assets and 100,000 representations;
- more than 100,000 resources and locators across single-file, sequence,
  ordered-part, and package structures;
- 1,000,000 typed metadata assertions;
- fan-in, fan-out, and a provenance chain 50 activities deep;
- 100,000 revisions whose events touch every asset;
- real BLAKE3-shaped resource and representation fingerprint evidence;
- a logical `media` root recorded on 90% of locators;
- 100 sparse unresolved representations whose resources have no locator;
- 9,700 snapshotted `org.postproject:transcode` activities by one exact tool,
  of which 97 snapshot a superseded input and are therefore stale.

The seed and cache path are explicit, and fixture construction is never part of
the measured operation:

```sh
POSTPROJECT_BENCH_SEED=postproject-0.4 \
POSTPROJECT_BENCH_FIXTURE=target/bench-fixtures/release-0.4.pproj \
  cargo bench --locked -p postproject-storage-sqlite --bench large_fixture
```

An existing cache is validated and reused. Delete that one explicit file to
regenerate it after the generator version or seed changes. Generation uses no
network, locale, or wall-clock input.

## Release 0.4 read-path baseline

The pre-query-API baseline was captured on 2026-09-24 at commit `7f835c0`
against the `postproject-0.4` fixture: 10,000 assets, 100,000 representations,
150,000 resources and locators, 1,000,000 metadata assertions, 151 activities,
and 100,000 revisions. The 494 MiB production was generated and measured with:

```sh
cargo bench --locked -p postproject-storage-sqlite --bench large_fixture
POSTPROJECT_BENCH_RUNS=3 \
  cargo bench --locked -p postproject-storage-sqlite --bench release_0_4_baseline
```

Each row reports three optimized samples. A read sample uses a fresh open
production handle, starts timing after the open, consumes the result, and runs
with the operating-system file cache warm. `open` measures the handle open
itself. The nested rows deliberately issue the existing per-parent calls; the
inventory row exercises the complete assets → representations → resources →
locators knowledge walk that the 0.4 domain-query work will change.

| Existing read path | Median | Minimum | Maximum |
| --- | ---: | ---: | ---: |
| Open production | 5.550 ms | 3.109 ms | 9.849 ms |
| Load all assets | 11.473 ms | 9.872 ms | 70.136 ms |
| Load representations for all assets | 5.820 s | 5.789 s | 8.728 s |
| Load resources for all representations | 8.275 s | 8.001 s | 10.660 s |
| Load locators for all resources | 4.429 s | 4.425 s | 5.775 s |
| Load metadata for all assets | 1.855 s | 1.645 s | 4.791 s |
| Query one metadata property | 42.369 ms | 39.812 ms | 56.494 ms |
| Load all activities | 1.439 ms | 1.264 ms | 2.373 ms |
| Activities producing one representation | 1.259 ms | 1.253 ms | 3.303 ms |
| Activities consuming one representation | 1.292 ms | 1.291 ms | 1.501 ms |
| Ancestors through a 50-deep chain | 0.369 ms | 0.302 ms | 1.033 ms |
| Descendants through a 50-deep chain | 0.305 ms | 0.301 ms | 0.326 ms |
| Complete inventory knowledge walk | 19.927 s | 19.884 s | 23.298 s |
| Latest revision | 0.042 ms | 0.037 ms | 0.828 ms |
| First 1,000 revisions | 1.184 ms | 1.184 ms | 3.470 ms |
| Events for one revision | 0.112 ms | 0.109 ms | 1.640 ms |

The run used Rust 1.98.1 on Linux 7.2.5, an AMD Ryzen 7 4800H, and a Samsung
970 EVO Plus NVMe SSD with Btrfs. The CPU governor was `ondemand`, frequency
boost was enabled, and the machine was not isolated. The spread in several
maximum samples is therefore scheduler noise; the medians are the comparison
baseline. These numbers precede 0.4 pagination and set-oriented read-path
changes and are informational, not release budgets.

## Release 0.4 domain-query pages

Every named query in schema 12 was measured for one page on the generator
version 2 fixture, captured on 2026-09-25 with the schema 12 changes applied on
top of commit `c6cecc0`:

```sh
cargo bench --locked -p postproject-storage-sqlite --bench large_fixture
POSTPROJECT_BENCH_RUNS=5 \
  cargo bench --locked -p postproject-storage-sqlite --bench release_0_4_queries
```

Each row reports five optimized samples of one page with a limit of 100, using a
fresh open production handle, timing after the open, and a warm file cache.
Items are the values returned; a continuation means another page exists.

| Query page | Items | Continues | Median | Minimum | Maximum |
| --- | ---: | :---: | ---: | ---: | ---: |
| Assets | 100 | yes | 0.135 ms | 0.111 ms | 0.146 ms |
| Representations of one asset | 10 | no | 0.749 ms | 0.683 ms | 1.172 ms |
| Resources of one representation | 2 | no | 0.228 ms | 0.224 ms | 0.450 ms |
| Locators of one resource | 1 | no | 0.111 ms | 0.106 ms | 0.116 ms |
| Representations under a logical root | 100 | yes | 3.233 ms | 2.957 ms | 6.474 ms |
| Unresolved media | 100 | no | 0.093 ms | 0.082 ms | 0.111 ms |
| Objects carrying a metadata property | 100 | yes | 0.709 ms | 0.652 ms | 1.826 ms |
| Metadata property with an exact value | 1 | no | 0.109 ms | 0.101 ms | 0.131 ms |
| Outputs by activity kind | 100 | yes | 0.128 ms | 0.124 ms | 0.140 ms |
| Outputs by exact tool | 100 | yes | 0.158 ms | 0.155 ms | 0.263 ms |
| Activities producing one representation | 1 | no | 0.454 ms | 0.444 ms | 0.577 ms |
| Activities consuming one representation | 100 | no | 8.838 ms | 8.505 ms | 9.121 ms |
| Ancestors through a 50-deep chain | 50 | no | 2.176 ms | 2.072 ms | 2.206 ms |
| Descendants through a 100-way fan-out | 100 | no | 3.706 ms | 3.645 ms | 3.807 ms |
| Stale artifacts, whole production | 1 | yes | 122.481 ms | 83.426 ms | 130.288 ms |
| Stale artifacts descending from one source | 1 | no | 1.009 ms | 0.952 ms | 1.274 ms |
| Objects changed in the last 1,000 revisions | 100 | yes | 4.789 ms | 4.711 ms | 9.019 ms |

Each page reads rows in proportion to its page, not to the production. Measured on
the same fixture at schema 11, without the derived query-support tables, the
unresolved-media page took 742 ms because it examined every membership to find
100 sparse matches. Outputs by exact tool took 37 ms, and a logical root holding
5 locators took 59 ms, for the same reason. Pages that return whole
representations or activities cost more per item than pages that return
identities. The stale-artifact page evaluates 100 candidate outputs, finds the
one stale output among them, and continues. The changed-object page reads the
journal suffix after its cursor.

The run used Rust 1.98.1 on the machine described in the read-path baseline
above, under the same unisolated conditions. These numbers are informational,
not release budgets.

## Quick baseline

A smoke baseline captured on 2026-09-22 with `--quick` at commit `1e92194`
produced these Criterion intervals:

| Workload | Observed interval |
| --- | ---: |
| Import and fingerprint 1,000 small files | 257.42–271.36 ms |
| Load 10,000 assets with identity evidence | 1.0835–1.0980 s |
| Resolve and fingerprint 3,000 same-size candidates | 84.330–86.524 ms |
| Commit one prepared import | 742.71–756.72 µs |

The run used Rust 1.98.1 on Linux 7.2.5, an AMD Ryzen 7 4800H, and a Btrfs
filesystem. Frequency boost was enabled and no power-policy controls were
applied. Because Criterion quick mode takes few samples and the machine was not
isolated, these numbers verify benchmark operation and provide an order-of-
magnitude baseline; they are not release gates or publication-quality claims.
