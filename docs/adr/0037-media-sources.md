# ADR 0037: Media sources for import and representations

- Status: Accepted
- Date: 2026-09-28

## Context

ADR 0003 gives every representation one of four content structures: a single
resource, an image sequence, ordered parts, or a package. Rust import accepts
all four, and the CLI imports recognized compound media as an asset's original.
The native surfaces did not. `pp_transaction_import_media` took one file path,
so a host could create an asset only from a single file. Four more functions
added a representation of each structure to an existing asset.

The Blender pilot records an image strip, a numbered image sequence, as one
asset. It could not. It had to import the first frame as the asset's original
and then add the sequence as a second original representation. The asset then
had a one-frame original that resolved on its own, although no strip used it.
Other hosts have the same need:

- a compositor or review tool reading an EXR or DPX plate as its source media;
- an editor importing a camera recording spanned over several files;
- an ingest tool importing a card package, such as AVCHD, whose sidecars belong
  to the media;
- an OpenAssetIO Manager registering a rendered sequence as a new asset.

Five functions described the same four structures in two places, and only the
single-file structure worked for import.

## Decision

A **media source** describes the content structure of a representation at its
present location: a single file, an image sequence, ordered parts, or a
package. The same source serves both operations that create a representation.

- **Import** creates an asset whose original representation has the source's
  structure.
- **Add representation** adds a representation of a given kind with the
  source's structure to an existing asset.

Both fingerprint the content and record a locator for each resource, as before.
The surfaces become:

- C: an opaque `pp_media_source_t`, made by `pp_media_source_create_file`,
  `pp_media_source_create_image_sequence`,
  `pp_media_source_create_ordered_parts`, or
  `pp_media_source_create_package`, and released with
  `pp_media_source_release`.
  `pp_transaction_import_media(transaction, source, display_name, out_asset_id,
  out_error)` and `pp_transaction_add_representation(transaction, asset_id,
  kind, source, out_representation_id, out_error)` replace the single-file
  import and the four `pp_transaction_add_*_representation` functions. A
  source is borrowed for the call and can be reused. Its inputs are checked
  against the content-structure rules when it is created; its files are
  inspected when a transaction uses it. This bumps the C ABI to version 34.
- C++: a copyable `postproject::MediaSource` value with the named constructors
  `file`, `imageSequence`, `orderedParts`, and `package`. It converts
  implicitly from a UTF-8 path string, the wrapper's form for every path, to a
  single-file source, so `importMedia(path)` still reads naturally. `Transaction::importMedia` and
  `Transaction::addRepresentation` take a source.
- Python: `MediaSource` values `FileSource`, `ImageSequenceSource`,
  `OrderedPartsSource`, and `PackageSource`. `Transaction.import_media` accepts
  a source or a path, and `Transaction.add_representation(asset_id, kind,
  source)` replaces the four `add_*_representation` methods.
- CLI: `media add` already imports recognized compound media; it is unchanged.
- Rust: `prepare_original_media` accepts a `MediaSource` enum whose variants are
  the existing source types; a path converts to a single-file source. The
  `prepare_*_representation` functions become one `prepare_representation`.

## Alternatives considered

- **Import variants beside the single-file import.** Three more import
  functions would pair with the four existing add functions, and each future
  structure would need two new functions. The two operations differ only in
  where the representation goes, not in how its content is described.
- **Creating an empty asset, then adding its original.** An asset always has
  an original representation (ADR 0003); an asset without one would exist
  between two calls in the same transaction and would need a new validation
  rule at commit.
- **Recognizing compound media from a path, as the CLI does.** Recognition
  guesses the structure from file names and layout. A host already knows it:
  Blender lists every frame of an image strip, and an editor knows a clip's
  spans. Recognition remains available in Rust and the CLI.

## Standards impact

None. The content structures are ADR 0003's; no stored form changes.

## Consequences

A host imports any content structure as an asset's original, with one import
and one add function for every structure. C and C++ callers of the removed
functions change their calls; the Python `import_media(path)` spelling and the
C++ `importMedia(path)` spelling keep working.
