# Package PostProject in a Flatpak

A Flatpak application links PostProject the same way as any other native
consumer: through the installed CMake package. PostProject is built from the
release source archive as one more module of the application's manifest. The
build is offline, as Flathub requires, and the application's own build never
invokes Cargo.

Each release since `0.4.0-alpha.1` publishes two files for this next to its
source archive:

- `postproject-VERSION-flatpak.json`, the module, pinned to that release's
  source archive and its SHA-256 checksum;
- `postproject-VERSION-cargo-sources.json`, the Rust crates the build needs,
  generated from `Cargo.lock` with the Flatpak project's
  `flatpak-cargo-generator`.

Copy both next to your manifest and list the module before the modules that
use PostProject:

```json
{
    "sdk-extensions": [
        "org.freedesktop.Sdk.Extension.rust-stable"
    ],
    "modules": [
        "postproject-0.5.0-alpha.1-flatpak.json",
        {
            "name": "my-application",
            "buildsystem": "cmake-ninja"
        }
    ]
}
```

The Rust SDK extension is needed only while building PostProject. It is not
added to the finished application. The module is this template, with the
release version and checksum filled in:

```{literalinclude} ../../../packaging/flatpak/postproject.json
:language: json
```

It installs the shared library, the C header, the C++ wrapper, and the CMake
and `pkg-config` metadata into `/app`, so `find_package(PostProject)` works in
later modules without further configuration. Manifests that clean up `/include`
and `/lib/cmake` after the build, as KDE application manifests do, still ship
the library.

## What is tested

The `Flatpak module` workflow renders the module for a source archive of the
revision under test. It builds the module on the KDE 6.10 SDK that Kdenlive's
manifest uses, then builds and runs the C and C++ smoke programs from
`tests/abi` against `/app`. Pull requests that change the module, the lockfile,
or the CMake package run it, and it also runs weekly. The quality checks
verify on every change that `postproject-cargo-sources.json` lists exactly the
crates in `Cargo.lock`.

When `Cargo.lock` changes, regenerate the Cargo sources with
[flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools):

```sh
python3 flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock \
  -o packaging/flatpak/postproject-cargo-sources.json
python3 tools/flatpak_module.py check
```

To try a revision that has not been released, render the module for a local
archive. The command also writes a small check application that builds it:

```sh
git archive --format=tar.gz --prefix=postproject-0.5.0-alpha.1/ \
  --output=target/flatpak/postproject-0.5.0-alpha.1-source.tar.gz HEAD
python3 tools/flatpak_module.py render --version 0.5.0-alpha.1 \
  --archive target/flatpak/postproject-0.5.0-alpha.1-source.tar.gz \
  --output target/flatpak
flatpak-builder --user --install-deps-from=flathub --force-clean \
  target/flatpak/app target/flatpak/org.postproject.FlatpakCheck.json
```

The [Kdenlive pilot](https://github.com/postproject-org/postproject-kdenlive)
is the application this module was made for.
