# Using PostProject

This section is for people using the PostProject CLI or an application that embeds PostProject. You do not need to understand the C ABI, SQLite schema, or Rust crate layout to use the model correctly.

## What a PostProject-enabled application should feel like

The main benefit should be simple: **media keeps its identity even when its storage situation changes**.

A production can know that a clip has moved, that a proxy belongs to an original, that an image sequence is one representation, or that an output came from particular inputs. Applications can share that knowledge without sharing their entire project formats.

## Media is not the same thing as a path

PostProject distinguishes the logical media from the place where bytes happen to be stored.

That means a file move does not automatically create a new asset. Instead, the application can resolve the existing resource at its new location, optionally verify it with fingerprint evidence, and continue using the same production identity.

If more than one candidate is plausible, PostProject keeps the result ambiguous. A host application should ask for a choice rather than silently attaching the wrong media.

## A production can contain several forms of the same asset

One logical asset may have an original, proxy, optimized version, or another derived representation. Compound representations can describe image sequences, ordered spans, and package-like media without inventing one asset per physical file.

For sequence and camera-media behavior, read {doc}`image-sequences-and-spanned-media`.

## Portable roots make storage machine-specific without making the production machine-specific

A production can name a logical media root such as `originals` or `rushes`. Each computer can then map that logical root to the path that exists on that machine.

This is preferable to turning one workstation's `/Volumes/...` or `D:\...` path into permanent production meaning.

The {doc}`portable-production-workflow` shows the full flow with CLI commands.

## Metadata says what is known; provenance says what happened

Metadata stores typed assertions and external vocabulary terms. Provenance records activities with inputs and outputs. These are related, but they answer different questions:

- **Metadata:** What do we know about this object?
- **Provenance:** What process connected these inputs and outputs?

Read {doc}`metadata-and-provenance` for a practical introduction.

## Local and explicit by design

A `.pproj` production is local production state. PostProject does not require a network service to understand the database, and standards-aware fields do not trigger automatic network lookups.

Inventory caches and machine-local discovery data are operational helpers. They are not the durable identity of the media.

## Recommended reading order

1. {doc}`../getting-started/core-model` — the asset → representation → resource → locator model.
2. {doc}`portable-production-workflow` — create a production, add media, move storage, resolve it again.
3. {doc}`image-sequences-and-spanned-media` — compound media.
4. {doc}`metadata-and-provenance` — inspection, metadata, and history.
5. {doc}`why-output-is-stale` — why a derived output may need rebuilding.

If you are writing an application rather than using one, continue with {doc}`../integrators/README`.

```{toctree}
:hidden:

portable-production-workflow
image-sequences-and-spanned-media
metadata-and-provenance
why-output-is-stale
```
