# ADR 0039: Platform wheels carrying the native library

- Status: Accepted
- Date: 2026-09-28

## Context

ADR 0008 made the Python package a platform-neutral wheel that loads a native
library the caller names, by argument or through `POSTPROJECT_LIBRARY`. A
Python application installs a native archive beside the wheel and points the
binding at it.

An application that embeds Python and loads plug-ins cannot work that way. The
Blender pilot showed why:

- A Blender extension bundles its Python dependencies as wheels. Blender
  installs the wheels of every extension into one shared `site-packages` and,
  when two extensions bundle wheels of the same name, keeps only the newest
  (`wheel_manager.py` in Blender 5.2). The pilot shipped the neutral wheel and
  its own copy of the native library. A second extension bundling a newer
  PostProject would replace the binding. The newer binding requires its own C
  ABI version exactly, so it would reject the first extension's library, and
  the first extension would stop working.
- An environment variable is process-wide. One plug-in setting
  `POSTPROJECT_LIBRARY` decides for every other plug-in in the same process.
- The Linux native library was built on the newest Ubuntu runner and required
  glibc 2.34. The Linux builds of Blender 5.2.2 and 5.3 alpha require only
  glibc 2.28 (the newest `GLIBC_` symbol version their executables import), so
  the library did not load on every system its host runs on.

Other embedding hosts share the first two constraints: DCC applications with
Python plug-in systems (Nuke, Houdini, Maya, Krita), pipeline launchers that
compose environments, and OpenAssetIO hosts loading a Manager plug-in.

## Decision

- **Platform wheels.** Each release publishes, beside the native archives, a
  wheel per platform that contains the binding and the native library of the
  same build: `manylinux_2_28_x86_64`, `macosx_11_0_arm64`, and `win_amd64`.
  Whichever of these wheels a host keeps, its binding and library match.
- **Loading order.** The binding loads an explicit `library_path`, else
  `POSTPROJECT_LIBRARY`, else the library installed inside the package. It
  still never searches the working directory or changes the platform loader
  path. A platform wheel therefore works with no configuration, and an
  explicit path or the variable still overrides it for development.
- **The neutral wheel remains** for platforms without a platform wheel and for
  applications that install the native package themselves. Without an explicit
  path or the variable it fails as before.
- **Linux builds for glibc 2.28.** The Linux native archive and wheel are
  built in the `manylinux_2_28` container, and CI checks that the library
  requires no newer glibc symbol version.
- **Hosts with shared plug-in environments** bundle the platform wheel and
  pass no library path. The integrator guide explains the newest-wins
  behavior and why a plug-in must not set the environment variable.

## Alternatives considered

- **Vendoring the binding inside each plug-in.** Each plug-in would carry its
  own binding and library under a private module name and never share it. That
  isolates plug-ins but asks every host integration to repackage PostProject,
  and two copies of different versions would then write one production with
  different schema expectations. The platform wheel keeps one binding per
  process, which is what the production file's single schema needs anyway.
- **Accepting any library with an equal or newer ABI version.** The C ABI is
  not backward compatible before 1.0 (ADR 0020), so a newer library cannot
  serve an older binding.
- **A compiled extension module.** It would also need per-platform wheels and
  add a build toolchain; ADR 0008's reasons for `ctypes` still hold.

## Standards impact

None. Wheel file names and platform tags follow the Python packaging
specifications (binary distribution format and platform compatibility tags,
including PEP 600 `manylinux_x_y` tags).

## Consequences

`pip install postproject` on a supported platform gives a working binding
without a native archive, and embedding hosts ship one file. The Linux library
loads on glibc 2.28 and newer. A plug-in may receive a newer binding than it
was built with when another plug-in bundles one; before 1.0 its Python API may
differ, which the guide states. ADR 0008's explicit-path rule gains the
package's own library as the last choice.
