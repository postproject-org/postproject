# Testing

The required local quality gate is:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
cargo deny check
```

On Windows, test `postproject-ffi` in a separate Cargo invocation from the rest
of the workspace. The public CLI executable and native library intentionally
share the installed name `postproject`, and concurrent MSVC links otherwise race
to write the same PDB file. CI runs both invocations sequentially, so no test
coverage is omitted.

A separate locked all-targets compile uses Rust 1.85.0, enforcing the stated
minimum supported Rust version independently of the stable-toolchain test matrix.

A separate Linux job invokes the real system `ffmpeg` against the shipped PPM
fixture and verifies that the reference executor publishes a non-empty
thumbnail. The ordinary cross-platform tests use deterministic fake executables
to cover capability absence, heartbeat delivery, crashes, bounded cleanup, and
atomic CLI completion without depending on FFmpeg being installed.

Tests use real temporary SQLite databases and filesystems, a checked-in schema-0
migration fixture, and the complete multi-asset relocation scenario. Dedicated
fuzz targets cover production opening, fingerprint input, C strings/errors,
external-identifier validation, and ID parsing outside standard CI. Tests must not require network access, user locale,
or wall-clock timing.

`cargo-deny` rejects wildcard dependencies, unknown sources, known advisories, and
licenses outside the repository's explicit permissive allowlist. Duplicate crate
versions are reported for review because platform support and MSRV constraints can
make them temporarily unavoidable.

The C ABI job builds the optimized shared library, compiles a standalone C11
consumer using only the public header, runs create/open/error operations, and
compares the exported dynamic symbols against `tests/abi/expected-symbols.txt`.

The native package matrix installs the platform library, optional static archive,
C and C++ headers, CMake package files, and `pkg-config` metadata into temporary
prefixes on Linux, macOS, and Windows. It then configures a separate C11/C++17
CMake production against each prefix, builds with warnings denied, and runs
lifecycle, identity, transaction commit/rollback, relocation,
structured-evidence, confirmation, move-ownership, and error-propagation checks.
The consumer configuration and build never invokes Cargo. Each platform package
is uploaded as a CI artifact. The same job builds and runs the C and C++
quickstarts from their installed locations and runs the installed Python
quickstart against the packaged media fixture.

Every code example in the documentation is an extract of a program that CI
runs. The `code-variants` Sphinx directive reads `[name]` ... `[/name]` comment
regions from every program in `docs/examples/{c,cpp,python,cli}` and
`docs/examples/rust/tests`; a region name is unique per surface, and the strict
documentation build fails when an example is neither present nor explicitly
marked unavailable for a surface. Each surface has one program per topic: the
end-to-end `guides` scenario (create, identify, describe, relink, add a
sequence, record provenance, query, and follow the revision feed) plus
`lifecycle`, `media`, `knowledge`, `provenance`, and `jobs`. The Rust programs
run under `cargo test --workspace`. The native package job configures
`docs/examples` against the installed prefix and runs every C, C++, Python, and
CLI program through CTest, each in a freshly prepared work directory. The CLI
scripts need `bash` and `jq` and are skipped on Windows.

`tools/check_example_coverage.py` fails when a function declared in the C
header, a public member function of a C++ wrapper class, or a public method of
a Python handle class appears in no example program, so a new operation cannot
ship without a tested demonstration.

Hosts compile the header-only C++ wrapper with their own compilers and flags.
`tools/check_cpp_header_warnings.py` compiles every C++ program against
`include/` as C++17 and C++20 at `-O2` with warnings as errors, without linking.
CI runs it with the current GCC and Clang in an Arch Linux container.
`tests/abi/cpp_isolated_calls.cpp` keeps wrapper calls that once raised a
warning in isolated functions, where optimizers warn most readily (ADR 0034).

Published documentation identifies historical scope by package release, C ABI,
or schema version. Planning labels remain confined to the untracked planning
briefs. A workspace test scans the published Markdown to keep that boundary
enforced.

| Concept page | Tested example region |
| --- | --- |
| Identity | `host-binding` |
| Assets, representations, resources, and locators | `resolve-asset` |
| External identifiers | `external-identifiers` |
| Metadata assertions | `metadata` |
| Production provenance | `provenance` |
| Dependency relationships | `dependency-queries` |
| Artifact knowledge and reproducibility | `artifact-knowledge` |
| Jobs and production work | `job-query-pages` |
| Revisions and semantic events | `revision-feed` |
| Rational time and ranges | `image-sequence` |

Standards boundaries is interpretive policy rather than a callable concept and
therefore documents explicitly why it has no executable variant.

The native sanitizer job rebuilds both consumers with AddressSanitizer and
UndefinedBehaviorSanitizer, enables leak detection, and runs their real lifecycle
and transaction workflows against the release shared library.

The CLI integration test creates a production, imports media, removes the known
file, discovers two byte-identical candidates under a configured root, verifies
an explicit ambiguous representation and resource result, confirms one
candidate, and reopens the production to observe the persisted locator. Commands
exchange JSON in this test so the machine-readable contract is exercised
alongside the domain workflow.

The storage E2E test moves a three-file media directory, verifies every known
locator is offline, resolves two unique resources, refuses to choose between two
byte-identical candidates for the third, explicitly confirms all choices in one
transaction, reopens the production, and resolves from persisted locators with no
roots supplied.

The 0.2 release E2E test creates four representations on one asset, attaches
typed metadata and an external identifier, records proxy-generation
provenance, and verifies one atomic semantic revision. It then moves the
single-file and compound media, discovers and confirms replacement locators,
retires the old locators and root, removes one sequence frame, and verifies the
resulting online/partial states and second revision after reopen.
