# API safety audit

Development audit against released `0.6.0-alpha.1`
(`a413227b9dd048f744fa7393d8927891ee93d5e2`, ABI 37, schema 17).
Candidate: `0.7.0-alpha.1` / Python `0.7.0a1`, C ABI 51, schema 19.
The implementation decisions below are resolved. Platform and host acceptance
remain separate checks in the [repository manifest](api-safety-repositories.md).

The authoritative C entry-point inventory is
[`expected-symbols.txt`](https://github.com/postproject-org/postproject/blob/main/tests/abi/expected-symbols.txt). All declarations
in the linked source modules, including constructors, setters and decoders,
belong to the rows below. The compatibility-family manifest covers evidence
subsets; it is not a complete safety inventory.

| Family / source | Decision and public contract | Regression evidence |
|---|---|---|
| [Identity and host references](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/id.rs) | Rust/C/C++ distinguish ID kinds; Python uses nominal hints over standard UUIDs. Checked object alternatives and production-qualified bindings preserve persisted text. Parsing establishes syntax, never existence. | Native `c_identities`, `cpp_identities`; Python `test_id_values`; installed typing positives and nine negatives |
| [Production and storage traits](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/storage.rs) | Explicit ownership and production selection; read facades cannot write. Request records are validated by the authoritative store. | Storage `production_lifecycle`, `read_sessions`; native/Python owner lifetime tests |
| [Content](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/content.rs) | Validated single-file, sequence, ordered and package cases. C uses selected-kind reads; C++/Python own checked alternatives. Sequence rate is separate from locator naming. | Storage `compound_media`; Python `test_content_values`; native smoke and decoder checks |
| [Resources and locators](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/resource.rs) | File/URI conversion, naming, roles and membership validate without requiring file presence. Explicit retirement requires a decision base. | Storage `locator_edits`, `compound_media`; media sequence tests; native examples |
| [Recognition and resolution](https://github.com/postproject-org/postproject/blob/main/crates/postproject-media/src/resolver.rs) | Read-only, bounded discovery/verification; preserve ambiguity. Setters reject invalid input immediately. Closed outcomes expose only applicable payloads; cancellation propagates. | Media resolver/sequence suites; FFI `resolution_payload_tests`; Python `test_resolution_values`; native smoke |
| [Metadata and registries](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/metadata.rs) | Owned, bounded values; unknown namespaces round-trip. Appends merge; replacement/removal require guarded decisions. | Storage `metadata`, `metadata_conflicts`; Python `test_metadata_edits`; C input/output examples |
| [Provenance](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/provenance.rs) | Validated edges and snapshots; output/provenance/job completion commit atomically. Cycles reject. Host clock facts confer no worker authority. | Storage `provenance`, `fingerprint_observations`; executor and host handoffs |
| [Dependencies and artifacts](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/artifact.rs) | Bounded traversal; guarded complete-set observations. Checked reason/issue alternatives preserve incomplete knowledge. | Storage `dependencies`, `dependency_edits`, `artifact_staleness`; FFI payload tests; Python `test_artifact_values` |
| [Jobs and leases](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/job_lease.rs) | Production-bound capabilities, private transport, authority time and exact 1 us–24 h durations. Every transition and commit checks current ownership. Freeing local ownership never writes. Observations contain no credentials. | Storage jobs with deterministic clocks; FFI lease/state tests; native `cpp_job_leases`; Python `test_job_leases`; real FFmpeg |
| [Reads, edits and receipts](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/read.rs) | Real pinned views carry bases into edits. Detached bases remain scoped. Commit returns its own receipt and every attempt is terminal; uncommitted Python contexts roll back. No-op commits create no revision. | Storage `read_sessions`, `commit_receipts`, `conflicts`; C/C++ lifetime/view contracts; Python view/receipt tests; CLI conflicts |
| [Revisions and waiters](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/revision.rs) | Bounded journal/event reads; head observations, durable cursors and wakeups retain distinct meanings. Cancellation/closure are explicit. | Storage `revisions`, `revision_waits`; Python `test_changes`; FFI waiter checks |
| [Queries and bounds](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/query.rs) | Cursors bind family/filter/production/view. Convenience collections reject overflow; storage, native results and resolution enforce item/byte budgets before copying. Explicit observation edits retain their input view. | Storage `domain_queries`, `convenience_bounds`, `root_pages`; Python `test_query_bounds`; FFI budgets; 10,000-asset paged benchmark |
| [Errors and FFI ownership](https://github.com/postproject-org/postproject/blob/main/crates/postproject-ffi/src/lib.rs) | Clear required outputs, contain panics, pair allocation/release, copy borrowed results and preserve structured conflicts. Unknown closed tags are unsupported; malformed payloads are corruption. | Direct FFI failure tests; Python `test_errors`; nine native contracts under ASan/UBSan |
| [Language projections and serialization](https://github.com/postproject-org/postproject/blob/main/include/postproject/postproject.hpp) | C++ retains Result and exception consumption. Python standard paths, UUIDs, Fraction rates and timedelta leases; installed typing metadata. CLI format 1 has stable error codes, scoped tokens and exact receipts. | C++ with/without exceptions; generated layouts/declarations; Python 121 tests (one skip); CLI workflows and extracted examples |
| [Packages and documentation](https://github.com/postproject-org/postproject/blob/main/CMakeLists.txt) | Installed native consumers invoke no Cargo. Matching headers/library/wheels; retained Rust 1.85, C11/C++17 and Python 3.11 floors. | Local native/wheel/typing and 72 extracted example checks; strict docs. Platform/full-host acceptance remains in the repository manifest |

Each row covers its constructors, setters, getters, decoders, convenience
overloads, raw/interchange helpers and corresponding CLI commands. The
[Rust exports](https://github.com/postproject-org/postproject/blob/main/crates/postproject-core/src/lib.rs), storage traits,
[C header](https://github.com/postproject-org/postproject/blob/main/include/postproject/postproject.h), C++ header,
[Python exports](https://github.com/postproject-org/postproject/blob/main/python/src/postproject/__init__.py) and
[CLI adapters](https://github.com/postproject-org/postproject/tree/main/crates/postproject-cli/src) are the surface inventories.
`check_example_coverage.py` checks every public C/C++/Python operation against
tested examples; symbol/layout and generated-declaration checks cover the C
boundary. These mechanical checks supplement the semantic regressions above.

ADRs 0045–0064 record the changed contracts and standards-impact reviews.
No standards mapping or remote synchronization behavior was added. The
0.6.x Result propagation promise and published artifacts remain intact; no
new stable family or cross-series binary replacement is claimed. Two real
renew/fail/cancel consumers are still absent from compatibility evidence.

## Baseline checks

On 2026-10-04, `cargo fmt --check`, workspace/all-features Clippy with warnings
denied, workspace/all-features tests, workspace Rustdoc and
`python3 tools/check_architecture.py` passed before source edits. Local logs:
`target/api-safety-baseline/{0,1,2,3,5}.log`.
The sandbox initially prevented `cargo deny check` from locking its advisory
database. The approved unrestricted check subsequently passed; a development
rerun also passed on 2026-10-05 (duplicate-version warnings remain informational).

Supported floors remain Rust 1.85, C11, C++17 and Python 3.11. CI checks native
Linux/macOS/Windows, installed CMake consumers, C/Rust/ctypes layouts, platform
wheels, documentation examples, sanitizers and the existing Flatpak route.
Local Linux checks do not establish the other platforms or real-host runs.
