# C quickstart

The native package installs the public C header, shared library, CMake package,
a buildable example, and a small deterministic media fixture. Rust and Cargo are
not required on the consuming machine.

Assuming PostProject is installed under `/opt/postproject`, build and run the
installed example with:

```sh
cmake \
  -S /opt/postproject/share/doc/postproject/examples/c \
  -B build/postproject-c-example \
  -DCMAKE_PREFIX_PATH=/opt/postproject \
  -DCMAKE_BUILD_TYPE=Release
cmake --build build/postproject-c-example --config Release
ctest --test-dir build/postproject-c-example \
  --build-config Release --output-on-failure
```

The test creates `c-example.pproj`, imports the installed
`sample-media.dat`, commits the transaction, and prints the asset ID, its
representation count, and the location of its original. Its production path
must not already exist.

For an application target, consume the same package normally:

```cmake
find_package(PostProject 0.4 REQUIRED CONFIG)
target_link_libraries(my_application PRIVATE PostProject::postproject)
```

## The example program

The installed example is the program below. It follows the ownership rules
that apply to every C integration:

- **Every handle is caller-owned.** Productions, transactions, result sets, and
  error handles are each released exactly once with their release function.
  Release functions accept `NULL`, so starting every handle as `NULL` gives one
  cleanup path for success and failure alike.
- **Errors are optional, owned handles.** A failed call returns its status code
  and, when `out_error` is not `NULL`, transfers an error handle. Stop at the
  first failure so an error that still has to be released is never
  overwritten. Branch on the status code; the message is diagnostic text and is
  borrowed from the error handle.
- **Only commit makes staged work durable.** Releasing a transaction that was
  not committed discards everything staged in it.
- **Result-set strings are borrowed.** A string read from a result set, such as
  a locator URI, stays valid only until the set is released.

```{literalinclude} ../../../examples/c/main.c
:language: c
```

The full contract for every function remains in `<postproject/postproject.h>`.
