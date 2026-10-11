# PostProject 0.8 exchange conformance

Candidate `0.8.0-alpha.1`, Python `0.8.0a1`, C ABI 52, schema 27,
experimental protocol major 1. These checks qualify local authority/passive
mirror behavior. They confer no stable wire or remote-service promise.

## Reconstructed knowledge

The installed candidate records real Blender 5.2.2 save, render and relink writes.
Typed helper proposals extend that same authority with compound media, exact
metadata, identifiers, roots, fingerprints, dependency sets and immutable
activity evidence. Two writer processes merge appends prepared from one base;
a stale destructive edit rejects. A deterministic storage-test worker adds
claim, renew, release/reclaim, failure, cancellation and output publication.

Two independent passive processes import different checkpoint positions and
apply contiguous suffixes to head 27. After duplicate application and reopen,
all 19 canonical section bodies and ordinary public observations agree:
three assets, three activities and seven jobs. Artifact staleness/reproducibility,
metadata order, original revisions/events and historical dependency/fingerprint
evidence agree. Three explicit caller mappings resolve the same local bytes;
the mappings do not enter portable knowledge.

{download}`Installed host evidence <evidence/release-0.8/host-convergence.json>`
pins source, runtime, artifact hashes and retained logs. Helpers and the private
test clock do not count as independent host or worker applications. Claimed
observations carry no usable capability; decoded checkpoint payloads and mirror
databases are checked for both text and binary lease identities.

## Failure and recovery evidence

The complete candidate workspace test run includes these focused suites:

| Contract | Executable evidence |
|---|---|
| Every mutation and persisted relation classified | `tools/check_exchange_inventory.py`; `docs/exchange-inventory.json` |
| Original changing/no-op/rejected outcome survives restart; changed intent rejects | `exchange_submissions`; local worker submission tests |
| Concurrent same-request submissions produce one original receipt/record | `submission_races` |
| Killed authority before/after native or proposal commit | `transaction::commit_crash` |
| Process exit before/after private checkpoint seal; exclusive destination promotion | checkpoint import tests; `checkpoint_import` |
| Process exit after chunk staging or committed replay | `mirror_replay` |
| Missing, reordered, truncated, altered duplicate or unsupported record rejects atomically | `mirror_replay`, exchange file and protocol codec tests |
| Compound/reference/history contradictions reject after valid rehashing | `checkpoint_integrity`; installed storage mutations below |
| Original intermediate media, dependency, provenance and job states survive replay | media/dependency/activity/job replay and checkpoint suites |
| Mirror transaction, worker and credential paths reject | installed C/C++/Python/CLI exchange contracts |
| Honest migration floor and retained earlier observations | `exchange_history`, `checkpoint_migration`, `exchange_resynchronization` |

Installed storage tests additionally pass **14 record and 9 checkpoint**
rehashed rejection cases. Hash validity therefore does not substitute for domain
validation. Failed imports publish no destination; rejected records preserve the
mirror head. The installed package checks are recorded in
{download}`candidate evidence <evidence/release-0.8/linux-candidate.json>`.

## Independent encoding and limits

Fixed fixtures include five framing cases, twelve proposals covering all 24
command alternatives, ten records and nine complete checkpoints. Python's
standard-library JSON encoder authored expected wire bytes independently of the
SDK. The pinned BLAKE3 binding is checked against upstream primitive vectors;
it is not a separate cryptographic implementation. Rust checks those fixed
bytes/digests and storage applies/reexports their portable bodies.

```sh
python3 tools/check_exchange_vectors.py
uv run --no-project --with blake3==1.0.10 \
  python3 tools/check_exchange_vectors.py --digests
```

The fixtures cover every metadata kind, exact integer/decimal/rate extremes,
Unicode, repeated unknown facts, split length/UTF-8 boundaries, all content
shapes, every public job state and opaque earlier-history archives. Negative
fixtures enforce duplicate-key, critical-field, scope, version and resource
bounds. The {doc}`schema-27-exchange-measurements` records 10,000-asset,
maximum-value and retention workloads against the same native implementation.

{download}`Storage fuzz evidence <evidence/schema-27/exchange-storage-fuzz.json>`
records two bounded 61-second ASan campaigns with actual valid checkpoint seeds.
They ran on the development build before the package version bump; native
implementation provenance is retained. Raw mutations often reject at integrity
checks, SQLite C was not separately instrumented, and these short campaigns do
not establish exhaustive or maximum-size coverage.
