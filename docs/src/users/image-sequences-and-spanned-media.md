# Image sequences and spanned media

Post-production media is often larger than a single file. An EXR sequence may contain thousands of frames; camera media may be split across several physical files or stored as a directory package. PostProject models those as **compound representations**, so applications do not need to pretend every file is an independent asset.

## Image sequences

A directory of numbered EXR, DPX, or TIFF frames can be added as one representation. The CLI requires the exact frame rate of a recognized sequence:

```sh
postproject media add production.pproj renders/shot010 --sequence-rate 24000/1001
```

The representation carries the compact structure of the sequence. Consumers can reason about the whole sequence while still knowing which frames/resources are required.

This is important for relocation: the host wants to find *the representation*, not independently relink thousands of unrelated assets.

The file names of a sequence — prefix, suffix, and frame-number padding — are recorded with its location, not with its identity. A sequence that was moved or renamed, for example by grading or delivery, is therefore found again by its content. Each candidate reports the naming its files have now, and confirming one records that naming with the new location:

```sh
postproject media resolve production.pproj SEQUENCE_ASSET_ID \
  --search-dir /mnt/graded --confirm file:///mnt/graded \
  --confirm-naming 'shot010-graded_%04d.exr'
```

`--search-dir` adds a machine-local directory to the search without recording it in the production. `--confirm-naming` is needed only when several namings were found in the same directory.

If a directory holds several unrelated numbered groups, `media add` reports how many it found instead of choosing one; add a path that identifies one layout.

## Spanned recordings

Numbered MOV, MXF, MP4, or MTS files in one directory are recognized as one recording split into ordered parts, so the spans stay one representation in their recorded order.

## Camera cards and package-like media

A directory containing structured camera media can be recognized as one meaningful media package instead of being flattened into a loose file list:

```sh
postproject media add production.pproj /Volumes/CARD
```

The built-in recognizer knows the AVCHD card layout (`PRIVATE/AVCHD/BDMV`): stream files become required members and clip information, playlists, and navigation files optional ones. Other formats need a host or adapter that describes them. PostProject stores the resulting representation/resource structure; it does not replace the format-specific reader or decoder.

## Companion files

Some media has sidecars or companion files that should be recognized together with the main item. Recognition of same-stem `.xmp`, `.xml`, or `.json` sidecars beside a file can be requested explicitly:

```sh
postproject media add production.pproj clip.mov --recognize-companions
```

Recognition is about describing storage structure. It does not imply that every neighboring file belongs to the media, and it should remain deterministic enough for an application to explain what was recognized.

## Availability is a property of the whole representation

For compound media, “the path exists” is not a sufficient availability test. A sequence may be missing frames; an ordered recording may be missing one span; a package may be incomplete.

PostProject therefore aggregates resource-level results into representation availability such as:

- **online** — required content is reachable;
- **partial** — some required content is missing;
- **offline** — the representation cannot currently be reached;
- **ambiguous** — resolution found more than one plausible answer;
- **error** — availability could not be determined cleanly.

A host application can use the accompanying issues/evidence to explain why the aggregate state was produced.

## Keep the application-level concept intact

The guiding rule is simple: **model media the way applications need to refer to it, not merely the way the filesystem happens to split it.**

For exact content-structure kinds and APIs, continue with {doc}`../integrators/compound-media` and {doc}`../concepts/assets-representations-locators`.
