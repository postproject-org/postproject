# Schema 27 exchange measurements

These Linux measurements qualify development ABI 52 with schema 27. They compare
ordinary workflows with released `v0.7.0-alpha.1`; they do not identify an accepted
0.8 package. [Raw samples and provenance](
evidence/schema-27/exchange-workload-measurements.json) name the source commits,
compiler, platform, binaries and verification results.

## Ordinary local work

The fixture uses the existing 10,000-asset synthetic-import shape: each asset has
one original single-resource representation, two eight-byte fingerprints and an
offline locator. Creation uses one native transaction. Reads visit every asset,
representation and resource. Append adds 1,000 assets in separate transactions.
Both builds use identical benchmark source, the same compiler and release profile.

| Operation | Released 0.7 median | Development median |
|---|---:|---:|
| Create 10,000 assets | 7.47 s | 9.18 s |
| Read all asset/resource evidence | 1.80 s | 1.80 s |
| Append 1,000 assets, total | 8.17 s | 9.46 s |
| Single-import transaction median | 5.70 ms | 6.68 ms |
| Single-import transaction p95 | 32.31 ms | 33.50 ms |
| Closed database at 10,000 assets | 19,243,008 bytes | 55,431,168 bytes |
| Creation peak RSS median | 20,288 KiB | 34,308 KiB |

Creation increased 23%; the median individual transaction increased 17%. Inspection
of the development store after 1,000 appends found 40,267,776 bytes of SQLite pages
for retained record chunks and 1,032,192 bytes for their manifests. Together they
account for almost all the 41.4 MB database-size difference. Temporary effect
fragments were empty; ordinary native commits retained no request outcomes.

The capture path checks effects against observations, writes canonical frames,
hashes chunks and retains them in the same writer transaction. This additional
work and I/O plausibly explain the measured overhead; table inspection is not a
CPU profile. History retention has linear disk cost and currently has no pruning.
The measurements do not justify adding an index service or changing domain APIs.

## Streaming and catch-up

Each of three fresh authorities exported a checkpoint at head 1 after the bulk
import. A new passive mirror imported it, then received 1,000 later native commits.
All file, checkpoint and replay limits used their defaults.

| Phase | Median time | Largest measured peak RSS |
|---|---:|---:|
| Export 81,349,793-byte checkpoint | 2.86 s | 22,136 KiB |
| Import checkpoint and validate retained history | 27.44 s | 22,136 KiB |
| Export 4,853,286-byte suffix | 0.33 s | 22,136 KiB |
| Apply 1,000 records | 9.51 s | 22,136 KiB |
| Verify the exact duplicate range | 0.45 s | 22,136 KiB |

After reopening, installed Python bounded reads found 11,000 assets,
representations and resources on each side. All 19 freshly exported canonical
section hashes matched. This verification ran separately from the timed phases.

One additional sample committed the maximum 15 MiB binary metadata value and
nesting depth 32. Its 65,291,882-byte checkpoint exported in 1.12 s and imported in
2.56 s; peak RSS was 115,108 KiB and 105,944 KiB respectively. A separate process
verified exact public values. Chunking bounds total-stream memory, but an
individual decoded value still requires memory proportional to its size.

One outcome-growth sample retained 1,000 changing requests, 1,000 empty no-op
requests and 1,000 missing-base destructive rejections, querying every original
outcome. Closed store sizes were 573,440 bytes initially, then 4,886,528,
5,627,904 and 6,279,168 bytes. The latter two groups created no revisions.

## Reproduce

Build `native_workflows` and `exchange_workflows` with `cargo build --release
--locked -p postproject-storage-sqlite --bench native_workflows --bench
exchange_workflows`. Copy only the native benchmark and its fixture into an
isolated released 0.7 source tree and build it with the same compiler. Run
`tools/measure_exchange_workflows.py --help` for the three executable paths and
new output directory, then use an installed wheel to run
`tools/check_exchange_measurements.py OUTPUT --output OUTPUT/verified.json`.

RSS is Linux `wait4` peak RSS per child, including the launcher's memory floor.
These are three warm-cache samples on a shared workstation; maximum-value and
outcome workloads have one sample. They establish fixture-specific costs, not
universal performance thresholds or maximum legal production size.
