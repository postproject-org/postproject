# Adopt media already known to a production

When a host encounters media, it can ask whether the shared production already
records that storage evidence. A match lets the host attach its own external
identifier to the existing asset instead of creating another logical asset.

Lookup is read-only. It never imports, adopts, merges, relinks, or changes a
locator. The host decides what a candidate means and performs any resulting
mutation in an explicit transaction.

## Find candidates

PostProject offers two bounded lookups:

- **Locator identity** matches one canonical current URI. For an image
  sequence, the identity also includes its exact prefix, suffix, and frame
  padding. A directory URI without that naming does not match a sequence.
- **Fingerprint evidence** matches the exact algorithm, algorithm version, and
  opaque value of a resource's current effective fingerprint. Foreign host
  algorithms are valid inputs when the host knows their precise semantics.

Each result identifies the matching resource, its parent representation, and
its owning asset. Follow `next_cursor` with the same query and limit until it is
absent.

```{code-variants} known-media-adoption
```

Convert native paths through PostProject before locator lookup. Comparing raw
path strings can miss URI escaping, symlink resolution, or platform-specific
canonicalization. Image-sequence callers must carry the sequence naming from
recognition or host state rather than guessing from the directory.

## Decide explicitly

Treat the result count as part of the workflow:

1. With no matches, the host may follow its normal import path.
2. With one match, the host may offer to adopt that asset.
3. With several matches, present all candidates or apply a host-owned decision
   that the user can inspect. Content equality alone does not prove that two
   logical assets are the same.
4. On adoption, attach the host's own qualified
   [external identifier](external-identifiers.md) to the selected asset.
5. If the host has learned another valid location, confirm that locator
   explicitly as described in [media resolution](media-resolution.md).

Do not replace another application's identifier. Different hosts can attach
different qualified identifiers to the same PostProject asset.

## Current evidence only

Normal locator lookup excludes retired locators. A retired path can later name
unrelated content, so historical paths are not safe adoption evidence.

Fingerprint lookup uses the resource's current effective recorded fingerprint,
not every past observation. After a new fingerprint observation replaces the
effective value, the former value no longer matches this query. Historical
audit information remains in revisions but does not silently affect adoption.

Neither lookup opens the media file or verifies that the current bytes still
match. Use [content verification](fingerprints-and-verification.md) when the
host needs current filesystem evidence.

## Concurrent first encounters

Two processes can encounter previously unknown media at the same time. Both
may see no candidates and commit separate logical assets. This is intentional:
PostProject does not impose global content deduplication, and identical bytes
can represent distinct logical production objects.

If avoiding duplicates matters to a host workflow, coordinate that decision at
the host level and re-run lookup before import. Do not silently merge assets
afterward.

## Scope of shared access

Multiple processes on one machine may open the same local `.pproj` file and
observe committed changes through the [revision feed](revision-feed.md). This
does not make a production file a network service: network filesystems,
remote revision streams, and access-control boundaries are outside this
package's shared-file contract.
