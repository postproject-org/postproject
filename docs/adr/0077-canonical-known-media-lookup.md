# 0077: Canonical known-media lookup inputs

Status: accepted for development 0.9.

## Decision

Python uses `find_known_media(source, *, limit, cursor=None)` on productions
and retained views. C++ uses `findKnownMedia(source, limit, cursor)` overloads.
Sources are ordinary file paths, exact locator identities or resource
fingerprints. Remove the separate public locator/fingerprint method names.

Strings are paths; URI inputs require `LocatorIdentity`. Paths use the native
canonicalization already used by import, including existence and symlink
checks, without hashing media bytes. Explicit locators need not exist locally.
Sequence lookup still requires the exact naming descriptor; a bare directory
path does not silently acquire sequence meaning. Python conversion uses the
reader's loaded library rather than constructing another loader.

Keep native C and Rust's typed evidence queries as the lower-level operations.
The CLI already accepts a file path for locator lookup. These surfaces do not
need a second query owner, runtime input hierarchy or changed ABI signature.

## Preserved behavior and impact

Lookup is read-only. Results retain asset, representation and resource IDs.
No-match, multiple candidates, continuation and cursor scope remain visible.
Content equality never authorizes adoption or identity merging. Values are
copied; retained-view lookup uses the original coherent knowledge and rejects
use after close. Invalid inputs leave storage unchanged.

Migrate Blender adoption and current SDK recipes/tests to the single method.
Historical evidence retains its recorded syntax. Package versions, ABI 53,
schema 27, CLI JSON 1 and protocol 1 remain unchanged by this projection change.

Standards-policy review: no identifier normalization, vocabulary mapping or
standards cardinality is introduced; canonical paths retain existing meaning.
