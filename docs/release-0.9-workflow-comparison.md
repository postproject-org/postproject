# Revised 0.9 workflow comparison

The [before program](evidence/release-0.9/python-abi53-before.py) is the exact
qualified `f456958c` example, run with its preserved ABI-53 wheel. The
[current program](examples/python/ergonomics.py) runs with the ABI-57 wheel.
Both complete all five tasks in fresh fixtures. The original
[0.8 comparison](src/integrators/api-migration-0.9.md) remains available.

| Task | Nonblank lines, before → current | Caller work removed or retained |
|---|---:|---|
| Import | 8 → 9 | Returns both identities; the additional assignment retains the asset ID for later tasks without another query |
| Adopt | 9 → 8 | One path lookup replaces fallible path-to-URI conversion and a locator wrapper; ambiguity and continuation checks remain |
| Relink | 21 → 17 | Verified selection carries evidence and the original base; removes first-member assumptions and copying locator fields |
| Output | 27 → 15 | One output/activity operation replaces separate staging and edge wrappers; uses the retained input ID, actual tool and parameter |
| Revisions | 5 → 5 | The explicit-view bounded scans remain useful and unchanged |

Counts include comments and assertions. They are supporting evidence rather
than an acceptance threshold. The before output also incorrectly recorded a
provenance input as an authored dependency. Removing that assertion corrects
meaning; it is not an API convenience saving. The current harness checks that
no dependency observation was invented. A separate scene fixture demonstrates
actual [authored references](src/integrators/dependencies.md).

Relinking now rejects partial verification, missing required members and
incomplete search. Selection survives disposal of the native result and view;
confirmation stages all locations against its original base. An existing
location is a guarded no-op. Commit remains explicit, conflicts remain visible,
and database guards do not guarantee unchanged external files.

No convenience operation adds a scan, subprocess or automatic retry. Paths
convert once; import/output preparation hashes before taking the write edit;
resolution releases its temporary production read before filesystem work.
Explicit retained views still have caller-controlled lifetimes. Python metadata
reads use scalars and immutable tuples, retaining tags for ambiguous meanings
and ordered repeated fields. Jobs and exchange live in discoverable modules;
their distinct ownership and deferred-intent capabilities are preserved.

All five surfaces have installed runnable recipes linked from the
[migration guide](src/integrators/api-migration-0.9.md). Native contracts and
Python regressions cover malformed inputs, incomplete/ambiguous selection,
copied evidence lifetime, terminal conflicts, no-op receipts, exact metadata and
effect capture/replay. Symbol counts and shorter snippets do not establish
those semantics.
