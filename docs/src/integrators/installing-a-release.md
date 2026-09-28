# Install a release without Cargo

Each tagged release publishes a source archive, native packages for Linux,
macOS, and Windows, Python wheels for those platforms, and a
platform-neutral Python wheel. A native package
contains the shared and static libraries, C and C++ headers, CMake and
`pkg-config` metadata, the `postproject` CLI, licenses, and tested examples.

Download the archive for the target platform and its adjacent `.sha256` file
from the [GitHub release](https://github.com/postproject-org/postproject/releases).
For example, on Linux:

```sh
sha256sum --check postproject-0.3.0-alpha.2-linux-x86_64.tar.gz.sha256
mkdir postproject-0.3.0-alpha.2
tar -xzf postproject-0.3.0-alpha.2-linux-x86_64.tar.gz \
  -C postproject-0.3.0-alpha.2
export PATH="$PWD/postproject-0.3.0-alpha.2/bin:$PATH"
export LD_LIBRARY_PATH="$PWD/postproject-0.3.0-alpha.2/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
postproject --version
```

Use `DYLD_LIBRARY_PATH` for an unpacked macOS archive. The Windows ZIP places
`postproject.exe` and `postproject.dll` together in `bin`; add that directory to
`PATH`. Applications can instead copy the archive into a conventional prefix
whose loader configuration already searches its `lib` directory.

CMake consumers point `CMAKE_PREFIX_PATH` at the extracted directory and link
`PostProject::postproject`. `pkg-config` consumers add its `lib/pkgconfig`
directory to `PKG_CONFIG_PATH`. Neither path invokes Cargo.

The Linux package and wheel are built for glibc 2.28 and load on any
x86-64 Linux with that glibc or newer. The macOS package and wheel need
macOS 11 on Apple silicon; the Windows ones need 64-bit Windows.

A platform wheel, such as
`postproject-0.4.0a1-py3-none-manylinux_2_28_x86_64.whl`, contains the
binding and the native library of the same build. Install it, and the binding
loads that library with no further configuration:

```sh
python -m pip install postproject-0.4.0a1-py3-none-manylinux_2_28_x86_64.whl
```

```python
from postproject import Production

production = Production.open("production.pproj")
```

The platform-neutral wheel, `postproject-0.4.0a1-py3-none-any.whl`, contains
only the binding. Install it beside a native package and give Python the
absolute library path when opening or creating a production:

```python
production = Production.open(
    "production.pproj",
    library_path="/opt/postproject/lib/libpostproject.so",
)
```

An explicit `library_path` always wins, then the `POSTPROJECT_LIBRARY`
environment variable, then the library inside a platform wheel. The binding
searches nowhere else.

The Python snippet is the same public call exercised by the installed
[Python quickstart](python.md). The CLI uses the Rust implementation directly;
it does not dynamically load the C library.
