# ADR 0030: Canonical locator spelling for hosts

- Status: Accepted
- Date: 2026-09-27

## Context

A locator is a URI. Import and confirmation store the `file:` URI of a
canonicalized path, spelled by the `url` crate. Hosts have their own URL types,
such as Qt's `QUrl`, Foundation's `NSURL`, JUCE's `URL`, Python's `pathlib` and
`urllib`, and OpenAssetIO's path utilities. Each spells some paths differently,
in percent-encoding of reserved and non-ASCII characters, drive-letter case, or
symbolic-link resolution. A host comparing its own URI with a stored locator
therefore gets false mismatches. The first real-host integration had to turn
every locator back into a path before comparing. No native surface could
produce the canonical spelling. The resolver also carried a second URI-to-path
conversion that reported different error kinds.

Strings the library returned for the caller to own had one release function,
named after host bindings. That was the only owned string output.

## Decision

- **One conversion, on every surface.** Every surface converts a path to its
  canonical locator (`canonical_file_uri`, `pp_file_path_to_locator`) and a
  local locator back to a path (`local_file_path`,
  `pp_locator_to_file_path`).
  - The forward conversion is exactly the one import and confirmation use.
    Symbolic links and relative components are resolved, so the path must
    exist.
  - The reverse conversion does not touch the filesystem.
  - The resolver uses the same function.
- **Hosts compare only PostProject spellings.** They compare locators only as
  strings obtained from PostProject. They never canonicalize URIs themselves,
  and PostProject defines no equality function over foreign spellings.
- **One release function for owned strings.** Every string returned through a
  `char **` output is released with `pp_string_release`, which replaces
  `pp_host_binding_release`. Strings borrowed from result sets are never
  released individually.

## Alternatives considered

- **A locator-equality function over arbitrary URI spellings.** Deciding that
  two foreign spellings name the same file requires the filesystem, because of
  links and case sensitivity. It would also make PostProject a general URI
  normalizer. Producing the canonical spelling of a path answers the host's
  real question, "is this my file?", without either.
- **Normalizing every stored locator more aggressively.** Locators may hold
  non-file URIs that must round-trip. More normalization would not help a host
  whose own spelling differs.
- **One release function per owned-string output.** This multiplies symbols
  without adding safety, because every such string is allocated the same way.

## Standards impact

The spelling follows RFC 8089 (`file` URI scheme) and the WHATWG URL Standard
as implemented by the `url` crate that import already uses. Nothing about the
stored form changes.

## Consequences

Hosts obtain stable, comparable locator strings on every surface, and each
file maps to one locator. C callers replace `pp_host_binding_release` with
`pp_string_release`.
