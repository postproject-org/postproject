# Fingerprints, verification, and inventory

[Resolution](media-resolution.md) answers *where* content can be reached. This
guide covers the related question of *what* the content is: recording that a
file's bytes changed, checking stored fingerprints against the files on disk,
finding media on storage that the production does not know yet, and recording
technical facts about a file.

All of these follow one rule: reading the filesystem never changes the
production by itself. A check reports; only an explicit transaction records.

## Record a new fingerprint observation

A fingerprint records what an object's content currently is. It can change over
time. When a host knows that a file was replaced in place, for example a
re-rendered plate or a re-conformed audio file, it *observes* the resource's
content again:

- The observation fingerprints the file, or the image-sequence directory, at
  the given path.
- It recomputes the fingerprint of every representation that uses the resource.
- It stages both in the transaction, and commit records them in one revision.
- The previous values move to history.

Hashing happens when the observation is staged, not while the production is
locked. The host never computes a representation fingerprint itself.

```{code-variants} fingerprint-observation
```

Recording an identical value is a no-op and creates no revision, so a host may
safely observe after every render or copy. A job worker that produces or
replaces media observes the new content before completing, so
[artifact evaluation](artifacts-and-staleness.md) sees current evidence.

A representation fingerprint is derived from its resources' fingerprints. A
resource fingerprint recorded without an observation, for example a value
computed elsewhere, marks each owning representation for recomputation. Until
its recomputed fingerprint is recorded as well, artifact evaluation treats the
affected knowledge as pending rather than current.

## Compute and compare content without recording

Computing a file's fingerprint needs no production. It returns exactly the
value that import records, which is useful for caching, for handing the value to
another process, or for checking a file before importing it:

```{code-variants} content-fingerprint
:::{no-variant} cli
The CLI has no command that only computes a fingerprint. `postproject media
verify-content` compares a file with a stored resource.
:::
```

Verifying compares the content at a path with a resource's stored fingerprints,
and never records anything. The fingerprint-observation example above verifies
before it observes. The result is one of three values:

- *matches*: the content is unchanged;
- *differs*: the content changed;
- *not comparable*: the resource has only fingerprints that PostProject cannot
  compute (see below), so nothing was compared.

## Keep a host's own content hashes

Many hosts already hash their media, for example an editor's head-and-tail
digest, a MAM checksum, or a camera's clip hash. Record such a value as a
resource fingerprint in its own domain, and PostProject keeps it byte for byte:

- **Name the algorithm after your application.** The algorithm identifier is
  1–64 ASCII letters, digits, `-`, or `_`, for example
  `example-editor-md5-head-tail`. Do not reuse a generic name such as `md5`
  unless the value is exactly that digest of every byte.
- **Increment the version whenever the computation changes.** Values from
  different versions are never compared with each other.
- **Keep PostProject's own fingerprint as well.** Importing a file records it.
  PostProject can only verify domains it computes itself; every other domain is
  *foreign* to it (ADR 0028).

Foreign fingerprints never prevent resolution. When a resource has no
fingerprint that PostProject can compute, the resolver works as it does for a
resource without fingerprints:

- It proposes files with the recorded name, and ranks them by size, parent path,
  and technical inspection.
- It never reports such a candidate as exact.
- It attaches `FingerprintNotVerified` evidence, whose detail lists the
  unchecked domains as `<algorithm> version <n>`.

The host should hash each such candidate with its own algorithm and confirm the
locator only when the values agree. Resolution never commits a candidate by
itself, so this check fits the normal confirmation step.

## Verify content during resolution

By default resolution trusts known locators whose files exist. Verification
additionally recomputes the fingerprints of content found at known locators and
reports a mismatch as evidence, so replaced or corrupted files are noticed.
Verification reads every byte it checks and is opt-in for that reason:

```{code-variants} verify-resolution
:::{no-variant} c
Content verification is not exposed through the C ABI. `pp_production_resolve_asset`
checks known locators and searches mapped roots without recomputing
fingerprints; use the CLI `media resolve --verify` or the Rust adapter.
:::
:::{no-variant} cpp
Content verification is not exposed through the C++ wrapper. `Production::resolve`
checks known locators and searches mapped roots without recomputing
fingerprints; use the CLI `media resolve --verify` or the Rust adapter.
:::
:::{no-variant} python
Content verification is not exposed through the Python binding.
`Production.resolve` checks known locators and searches mapped roots without
recomputing fingerprints; use the CLI `media resolve --verify` or the Rust
adapter.
:::
```

A verification mismatch is a report. Record the new observation, as shown
above, only after the integration has decided the new bytes are correct.

`FingerprintMismatch` means that PostProject compared the content in one of its
own domains and the content differs. If a resource only has foreign
fingerprints, verification reports the known locator as online with
`FingerprintNotVerified` evidence, because nothing was compared. The host's own
check decides that case.

## Inventory storage

An inventory scan walks the mapped media roots and classifies what it finds:
known resources that are online, partial, missing, or changed since they were
recorded; new media that no representation references yet; duplicate and
ambiguous relink candidates; and roots that are unmapped or unavailable on this
machine. It never changes the production. An
optional sidecar cache lets repeated scans skip fingerprinting unchanged files;
the cache is machine-local and holds no production knowledge.

```{code-variants} inventory-scan
:::{no-variant} c
Inventory scanning is not exposed through the C ABI. Run `postproject media
inventory` from a host tool, or resolve individual assets with
`pp_production_resolve_asset`.
:::
:::{no-variant} cpp
Inventory scanning is not exposed through the C++ wrapper. Run `postproject
media inventory` from a host tool, or resolve individual assets with
`Production::resolve`.
:::
:::{no-variant} python
Inventory scanning is not exposed through the Python binding. Run `postproject
media inventory` with `subprocess`, or resolve individual assets with
`Production.resolve`.
:::
```

## Record technical inspection

The optional inspection adapter runs `ffprobe` as a subprocess and records a
bounded technical summary of the container and its streams as one structured
metadata assertion in the `https://postproject.org/ns/technical-media/1`
vocabulary, attached to the representation. A missing `ffprobe` is reported as a capability gap, not an
error, and PostProject never ships or downloads the executable.

```{code-variants} media-inspection
:::{no-variant} c
Inspection is not exposed through the C ABI. A host that already inspects media
can record its own findings as typed [metadata](metadata-vocabularies.md) with
`pp_transaction_add_metadata_value`.
:::
:::{no-variant} cpp
Inspection is not exposed through the C++ wrapper. A host that already inspects
media can record its own findings as typed [metadata](metadata-vocabularies.md)
with `Transaction::addMetadataValue`.
:::
:::{no-variant} python
Inspection is not exposed through the Python binding. A host that already
inspects media can record its own findings as typed
[metadata](metadata-vocabularies.md) with `Transaction.add_metadata`.
:::
```

Inspection results are observations of one file at one time. They do not
replace the fingerprint as evidence of identity.
