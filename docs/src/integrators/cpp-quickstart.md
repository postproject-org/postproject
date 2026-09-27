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
`sample-media.dat`, commits through the RAII transaction wrapper, and verifies
that the asset owns one representation. Its production path must not already
exist.

Use the exported package target from an application:

```cmake
find_package(PostProject 0.3 REQUIRED CONFIG)
target_link_libraries(my_application PRIVATE PostProject::postproject)
target_compile_features(my_application PRIVATE cxx_std_17)
```

Include `<postproject/postproject.hpp>`. Owned handles are move-only and clean
themselves up. Every fallible operation returns a `postproject::Result<T>`
holding either the value or a `postproject::Error`, which has a typed `code()`
and a diagnostic `message()`. The header never throws on its own, so it builds
in projects compiled with `-fno-exceptions`, such as KDE applications, game
engines, and plugin SDKs:

- **Check a result explicitly.** Call `ok()`, then read the value with `*` or
  `->`, or read `error()`.
- **Call `value()` where exceptions are enabled.** It throws
  `postproject::Exception` on failure. Without exceptions it prints the error
  and aborts, so code built with `-fno-exceptions` should always check `ok()`
  first.

`MetadataInput` and `ResolutionOptions` record an invalid argument instead of
failing when they are built. The operation that consumes them then returns
that error.
