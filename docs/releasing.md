# Release checklist

PostProject is pre-release software. Release candidates use the workspace
version from `Cargo.toml`, the corresponding PEP 440 version from
`python/pyproject.toml`, schema version from
`postproject-storage-sqlite::CURRENT_SCHEMA_VERSION`, and ABI version from
`postproject-ffi::ABI_VERSION`.

Before tagging a release:

1. Date the release heading in `CHANGELOG.md` and confirm every public ABI
   change is recorded. The release workflow uses that section as its notes.
2. Run `python tools/check_versions.py`, verify all workspace and fuzz
   dependency versions are locked, and confirm both `cargo deny` policies pass.
   The version check also rejects stale package filenames in current guides.
3. Run formatting, Clippy, tests, rustdoc, the C/C++ installed consumers, and the
   fuzz-target compile audit exactly as CI does.
4. Run the Criterion suite and record commit, toolchain, OS, CPU, storage, and
   filesystem when publishing numbers.
5. Confirm `tests/abi/expected-symbols.txt`, ABI version, and schema version are
   intentional.
6. Configure the native package from locked release outputs, install it into an
   empty prefix, and run the installed C, C++, and Python quickstarts using only
   that prefix and the built wheel.
7. Confirm the install contains shared and static libraries, C/C++ headers,
   CMake and `pkg-config` metadata, the quickstarts and fixture, stewardship
   policy, README, changelog, and both licenses.
8. Push the prepared main branch and inspect every individual CI job. Create a
   signed `v<package-version>` tag at the qualified commit, verify it with
   `git tag -v`, and push that tag. The release workflow verifies the tag
   against Rust, CMake, and Python package versions before publication.
9. Confirm the workflow publishes the conventional source tarball, Linux,
   macOS, and Windows native archives, a platform wheel for each of them, and
   the platform-neutral Python wheel, together with a SHA-256 checksum for
   each artifact. Never rebuild an artifact after tagging.

Pushing the tag publishes a GitHub prerelease and its immutable documentation;
the documentation workflow also advances `latest`. It does not upload packages
to crates.io or PyPI.

The maintainer approved a documentation-only correction to the published 0.7
installation and Flatpak examples. Their source is pinned to `bdaa808` in the
documentation workflow; the release tag and package assets are unchanged.

Linux, macOS, and Windows package artifacts are produced from
`cargo build --locked` and the same CMake install rules exercised on every
push. Windows packaging includes the DLL and its matching import library by
passing them as `POSTPROJECT_RUNTIME_LIBRARY` and `POSTPROJECT_LIBRARY`; macOS
packaging must preserve the dylib install name expected by the CMake target.
The Linux archive and wheel are built in the `manylinux_2_28` container, and
the workflow fails if the library imports a glibc symbol version newer than
2.28 (ADR 0039); macOS builds set `MACOSX_DEPLOYMENT_TARGET=11.0`. Each
platform wheel is the platform-neutral wheel with that build's library added by
`tools/build_platform_wheel.py`, and is tested by running the quickstart from a
fresh virtual environment without a library path. The platform-neutral wheel
still requires a native package; the binding never performs an implicit
dynamic-library search.
