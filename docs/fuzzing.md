# Fuzzing

Boundary-heavy inputs have dedicated `cargo-fuzz` targets:

- `production_opening` writes arbitrary bytes as a production file and exercises safe
  migration/opening failure paths;
- `fingerprint_input` exercises algorithm, version, and opaque-value validation;
- `c_abi_strings` passes arbitrary NUL-terminated bytes through the C string and
  error boundary, then releases every returned handle;
- `external_identifier_validation` splits arbitrary bytes into a scheme, value,
  and qualifier and exercises external-identifier validation;
- `id_parsing` exercises all strong-ID text parsers.

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
