# Assets, representations, resources, and locators

An **asset** is a logical production object. It is not a pathname.

A **representation** is one concrete realization of an asset, such as a camera
original, editorial proxy, mezzanine transcode, or render sequence. A
representation is not necessarily one file. It has an explicit content
structure describing how storage resources form usable media.

A **resource** identifies a storage-level component. A single-file movie uses
one resource, while spanned recordings and packages use several. A regular
image sequence uses one compact patterned resource instead of creating one
database object per frame. File facts and storage-level fingerprints belong to
resources.

A **representation fingerprint** combines the content structure with its
resource-fingerprint evidence. It excludes PostProject object IDs and locators,
preserves order for spanned media, treats package member order as irrelevant,
and hashes an image sequence's compact descriptor without visiting every
frame during representation aggregation. Neither fingerprint includes an image
sequence's file names, so a renamed copy has the same identity. The sequence resource contributes
deterministically sampled member-content evidence. Its confidence cannot exceed
the resource evidence it aggregates: a sampled resource fingerprint does not
become a full-content identity claim.

A **locator** describes where or how a resource can currently be accessed. One
resource may have several locators, such as paths through different mounts.
Moving content changes its locator rather than its resource, representation, or
asset identity. For an image sequence the locator names a directory together
with the naming of the files there: their prefix, suffix, and frame-number
padding. Renaming a sequence therefore changes only its locator, and a copy of
the sequence under other names is another locator of the same resource.

Content structures distinguish:

- one resource;
- a compact image sequence with a frame domain, exact rate, and known gaps,
  whose file naming belongs to each locator;
- ordered resources, such as a spanned recording; and
- a package whose required and optional members have extensible roles.

Availability is evaluated for the complete representation. All required
content being resolvable is **online**; some required content being available is
**partial**; none being available is **offline**; and multiple plausible
resolutions are **ambiguous**. Missing optional package members can be reported
without making otherwise usable media partial.

This separation is why PostProject can retain an asset's identity and metadata
when storage paths change, and why one missing image-sequence frame is not
mistaken for a fully online representation.

## Across public surfaces

Resolution reports candidates without mutating the production or silently
choosing among ambiguous locations:

```{code-variants} resolve-asset
```

Reading a representation returns its content structure, members, resources,
fingerprints, and locators as separate facts:

```{code-variants} representation-structure
```
