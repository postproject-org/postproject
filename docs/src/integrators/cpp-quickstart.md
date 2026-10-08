# C++ quickstart

The C++17 interface is a header-only RAII wrapper over the installed C ABI. It
does not link to Rust APIs or add a second native library.

Assuming PostProject is installed under `/opt/postproject`, build and run the
installed example with:

```sh
cmake \
  -S /opt/postproject/share/doc/postproject/examples/cpp \
  -B build/postproject-cpp-example \
  -DCMAKE_PREFIX_PATH=/opt/postproject \
  -DCMAKE_BUILD_TYPE=Release
cmake --build build/postproject-cpp-example --config Release
ctest --test-dir build/postproject-cpp-example \
  --build-config Release --output-on-failure
```

The test creates `cpp-example.pproj`, imports the installed
`sample-media.dat`, commits through the RAII transaction wrapper, and prints
the asset's representation count. It then shows that invalid metadata
input is reported by the operation that consumes it. Its production path must
not already exist.

Use the exported package target from an application:

```cmake
find_package(PostProject 0.7 REQUIRED CONFIG)
target_link_libraries(my_application PRIVATE PostProject::postproject)
target_compile_features(my_application PRIVATE cxx_std_17)
```

Include `<postproject/postproject.hpp>`. Owned handles are move-only and clean
themselves up. Every fallible operation returns a `postproject::Result<T>`
holding either the value or a `postproject::Error`, which has a typed `code()`
and a diagnostic `message()`. The header never throws on its own, so it builds
in projects compiled with `-fno-exceptions`, such as KDE applications, game
engines, and plugin SDKs:

- **Pass a failure on with the propagation macros.** In a function that
  returns a `Result`, `POSTPROJECT_TRY_ASSIGN(auto production,
  Production::open(path));` returns the error of a failed call to the caller
  and otherwise declares the value. `POSTPROJECT_TRY(transaction.commit());`
  does the same for an operation without a value. The enclosing function may
  return any type implicitly constructible from `postproject::Error`, so a
  host's own error type can take part by adding such a constructor.
  `POSTPROJECT_TRY_ASSIGN` expands to several statements: use it only directly
  inside a block, at most once per line.
- **Check a result explicitly where the failure is handled.** Call
  `has_value()`, then read the value with `*` or `->`, or read `error()`.
- **Call `value()` where exceptions are enabled.** It throws
  `postproject::Exception` on failure. Without exceptions it prints the error
  and aborts, so code built with `-fno-exceptions` should always check
  `has_value()` first.

`Result` uses the member names of C++23 `std::expected<T, Error>`:
`has_value()`, `error()`, `value()`, `value_or()`, and the combinators
`and_then()`, `transform()`, and `or_else()`. Unlike `std::expected`, its error
type is always `postproject::Error`, and it is constructed from an `Error`
directly rather than through `std::unexpected`.

Create cancellation tokens and resolution options with their `create()`
factories. Options setters return `Result<void>` immediately; a failed setter
preserves the previous settings. Use propagation macros or `.value()` to check
each call. Metadata request values are validated when a mutation consumes them.

## The example program

The installed example is the program below. `import_media` passes every
failure on with the propagation macros and builds with or without exceptions;
returning early from it destroys the open transaction, which discards the
staged import. `main` checks its result explicitly, then uses `value()` inside
a `try` block, and `rejects_invalid_metadata` passes an invalid decimal to
`addMetadataValue`, which returns an `invalid_argument` error.

```{literalinclude} ../../../examples/cpp/main.cpp
:language: cpp
```
