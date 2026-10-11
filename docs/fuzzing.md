# Fuzzing

Boundary-heavy inputs have dedicated `cargo-fuzz` targets:

- `production_opening` writes arbitrary bytes as a production file and exercises safe
  migration/opening failure paths;
- `fingerprint_input` exercises algorithm, version, and opaque-value validation;
- `c_abi_strings` passes arbitrary NUL-terminated bytes through the C string and
  error boundary, then releases every returned handle;
- `external_identifier_validation` splits arbitrary bytes into a scheme, value,
  and qualifier and exercises external-identifier validation;
- `id_parsing` exercises all strong-ID text parsers;
- `exchange_document` checks strict JSON/framing, proposals, outcomes and
  portable media, metadata, provenance, dependency and job values, including
  checkpoint manifests/chunks and retained history evidence;
- `exchange_files` imports sealed checkpoint files and atomically applies record
  files to real passive stores, checking failed-import cleanup, unchanged heads
  after rejection, reopen, reexport and passive write rejection.

Install `cargo-fuzz`, then run one target with a nightly Rust toolchain:

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run production_opening -- -max_total_time=60
```

List all targets with `cargo +nightly fuzz list`. Corpora and crash artifacts are
local under `fuzz/corpus` and `fuzz/artifacts` and are ignored until a minimized
regression input is deliberately promoted into a normal test fixture. Fuzzing is
not part of standard CI because sanitizer-driven campaigns are intentionally
long-running; CI format-checks the fuzz package and lints its targets with
Clippy on every change.

`libfuzzer-sys` is used rather than a custom harness. Its combined permissive
license expression is `(MIT OR Apache-2.0) AND NCSA`. The isolated fuzz package
has a matching dependency-policy configuration; audit it with:

```sh
cargo deny --manifest-path fuzz/Cargo.toml --config fuzz/deny.toml check
```

The [schema-24 decoder campaign](evidence/schema-24/decoder-fuzz.json) records
its exact scope, compiler, checksums and successful sanitizer rerun. It predates
incremental frame decoding and passive replay.
The [incremental-frame campaigns](evidence/schema-24/frame-decoder-fuzz.json)
also exercise raw framed bodies and portable production/version headers; storage
replay and checkpoint import are outside their scope.
The [schema-27 protocol campaigns](evidence/schema-27/protocol-decoder-fuzz.json)
add all current portable value families and independently authored proposal and
genesis-checkpoint seeds. Their input and storage limits remain explicit.
The [schema-27 storage campaigns](evidence/schema-27/exchange-storage-fuzz.json)
exercise file decoding and SQLite reconstruction with fixed nonempty checkpoints
and a complete Blender/job history seed. Inputs are bounded to 2 MiB; these
campaigns do not qualify maximum legal values or scale workloads.
The [rehashed storage cases](evidence/schema-27/rehashed-storage-files.json)
separately alter domain facts and recompute file integrity. Installed native
import/apply checks reject false history, snapshots, job decisions and semantic
guards while preserving the prior mirror or leaving no imported destination.
