# Compound-media integration

A host creates one representation for a sequence, span, or package. It should
not import every member as an unrelated asset. The representation owns a
content structure; its resources carry content evidence and one or more
locators.

## Media sources

A **media source** describes the content structure of a representation at its
present location: a single file, an image sequence, ordered parts, or a
package (ADR 0037). The same source serves both operations that create a
representation:

- **import** creates an asset whose original representation has the source's
  structure; and
- **add representation** adds a representation of a chosen kind, with the
  source's structure, to an existing asset.

Both fingerprint the content and record a locator for each resource. A host
already knows the structure of what it imports: an image strip lists its
frames, and an editor knows a clip's spans. A plain path is a single-file
source on every surface.

| Surface | Single file | Image sequence | Ordered parts | Package |
| --- | --- | --- | --- | --- |
| C | `pp_media_source_create_file` | `pp_media_source_create_image_sequence` | `pp_media_source_create_ordered_parts` | `pp_media_source_create_package` |
| C++ | a path, or `MediaSource::file` | `MediaSource::imageSequence` | `MediaSource::orderedParts` | `MediaSource::package` |
| Python | a path, or `FileSource` | `ImageSequenceSource` | `OrderedPartsSource` | `PackageSource` |
| Rust | `MediaSource::File` | `MediaSource::ImageSequence` | `MediaSource::OrderedParts` | `MediaSource::Package` |

In C, `pp_transaction_import_media` and `pp_transaction_add_representation`
borrow a `pp_media_source_t` only for the call, so one source may serve several
calls; release it with `pp_media_source_release`. The CLI describes an added
representation with a JSON source file and recognizes the structure of
imported media, as shown below.

## Import an image sequence as an original

A compositor reading an EXR plate, or an editor importing an image strip,
records the sequence as one asset whose only original representation is the
sequence. The example imports a render directory with one frame known to be
missing and reads back the asset's single original:

```{code-variants} import-sequence
```

## Add an image sequence

An image-sequence representation is described compactly: directory, file
naming (prefix, suffix, and frame-number padding), first and last frame, frame
step, an exact rational frame rate, and any frames already known to be missing.
The directory and naming become the sequence's first locator; the frames, rate,
and missing frames become its descriptor. File names belong to where a sequence
is, not what it is (ADR 0038), so another copy may name the same frames
differently. The example adds a derived render sequence to an existing asset
and reads back the stored descriptor:

```{code-variants} image-sequence
```

## Add a proxy or other single-file representation

A proxy, an optimized mezzanine, or a derived render is another representation
of the same asset, not a new asset. The example adds a single-file proxy to the
imported asset; the file becomes a new resource with a confirmed locator and a
content fingerprint:

```{code-variants} add-representation
```

A path converts to a single-file source, so a proxy needs no other source
type. The representation kind (original, proxy, optimized, or derived) says
how the representation relates to the asset. Why it exists — which activity
produced it from which input — is recorded separately as
[provenance](provenance.md).

## Add ordered parts

A camera recording spanned across several files is one representation whose
members are ordered. Each member carries an open-world role and is required
unless marked optional; the order of the list is significant and preserved:

```{code-variants} ordered-parts
```

## Add a package

A package groups files that belong together without an order, such as an
essence file and its metadata sidecar. Optional members may be missing without
making the representation unavailable:

```{code-variants} package-representation
```

## Read the stored structure

After commit, enumerate representations rather than retaining a private host
index. Each representation exposes:

- the content-structure kind;
- ordered members, open-world roles, and requiredness;
- the compact sequence descriptor and the frames known to be missing;
- resources and their fingerprints;
- resource locators, each with the naming of its sequence files for an image
  sequence; and
- representation fingerprints separately from resource fingerprints.

The example reads one page of an asset's representations and walks that
structure:

```{code-variants} representation-structure
```

## Availability

Read a sequence's file names from the locator you use, not from the
representation: a sequence renamed and relinked has one locator per naming.

[Resolution](media-resolution.md) returns one aggregate availability value
plus resource results and issues. Missing sequence frames are sorted individual
frame numbers. Optional package members may produce issues but do not reduce
availability. Never choose one ambiguous candidate in integration code; present
the candidates to the user and persist only an explicit confirmation.

## Recognition

The Rust media adapter additionally recognizes compound media from filenames
and layouts: numbered image groups, numbered camera spans, same-stem sidecars,
and the checked AVCHD card layout. The CLI reaches the same adapter when
`media add` receives a directory, and with `--recognize-companions` for
sidecars:

```{code-variants} media-recognition
:::{no-variant} c
Recognition is not exposed through the C ABI. Describe the structure
explicitly with `pp_media_source_create_image_sequence`,
`pp_media_source_create_ordered_parts`, or `pp_media_source_create_package`,
and import or add it as shown above.
:::
:::{no-variant} cpp
Recognition is not exposed through the C++ wrapper. Describe the structure
explicitly with `MediaSource::imageSequence`, `MediaSource::orderedParts`, or
`MediaSource::package`, and import or add it as shown above.
:::
:::{no-variant} python
Recognition is not exposed through the Python binding. Describe the structure
explicitly with `ImageSequenceSource`, `OrderedPartsSource`, or
`PackageSource`, and import or add it as shown above.
:::
```

AVCHD recognition assigns open-world PostProject roles for essence,
clip-information, playlist, and navigation members. These labels describe the
adapter's preservation model; they do not claim conformance validation or
interpret vendor metadata.
