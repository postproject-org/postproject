# Integrator guide

This section is for developers adding PostProject to a host application, plugin, service, or pipeline tool.

The fastest successful integration is usually **smaller than the whole PostProject feature set**. Begin with durable identity and media resolution, prove that the host can reopen the same production objects, then add richer knowledge only when the application has a use for it.

## Recommended integration path

### 1. Install and open a production

Choose a public surface:

- **C** — the native ABI and lowest common denominator;
- **C++17** — convenience wrapper over the native API;
- **Python** — binding over the C ABI;
- **CLI** — useful for scripting, diagnostics, fixtures, and integration tests.

Start with {doc}`installing-a-release`, then choose {doc}`c-quickstart`, {doc}`cpp-quickstart`, or {doc}`python`.

### 2. Create or open media identities

Follow {doc}`first-production`. It also shows how to reopen a production, commit or roll back a transaction explicitly, and handle errors on each surface. Store the returned PostProject identity in the host's own project/object model where appropriate, ideally as a {doc}`host-object binding <host-object-bindings>`.

Do not substitute a filesystem path for that identity. The entire portability model depends on those being different concepts.

### 3. Make media resolution part of host behavior

Implement {doc}`media-resolution` before building elaborate metadata features. A useful integration should survive media moving to a different mount or workstation.

The host should surface ambiguity rather than silently accepting the first candidate. {doc}`fingerprints-and-verification` covers what to do when content itself changes: recording a new fingerprint, verifying content during resolution, and scanning storage for media the production does not know yet.

### 4. Support the media structures your application actually uses

If the host handles VFX or camera media, continue with {doc}`compound-media` so sequences, spans, packages, and proxies remain one representation each instead of becoming one logical asset per file.

### 5. Add production knowledge incrementally

Use the pieces that solve real host problems:

- {doc}`external-identifiers` for identifiers owned by other systems;
- {doc}`metadata-vocabularies` for structured, typed metadata;
- {doc}`provenance` for activity history and input snapshots;
- {doc}`dependencies` for files that reference other production objects;
- {doc}`artifacts-and-staleness` for knowing whether a generated artifact is still current;
- {doc}`jobs-and-workers` when the host coordinates durable production work;
- {doc}`reference-executor` as an example of the execution boundary, not as a requirement;
- {doc}`bounded-queries` for scalable enumeration and traversal;
- {doc}`revision-feed` for reacting to changes, including changes made by other processes;
- {doc}`openassetio-publishing` when an OpenAssetIO host publishes rendered media through the PostProject Manager.

## Every guide shows every surface

Each task guide shows the operation in C, C++, Python, Rust, and the CLI. Choose a language with the tabs or with the **Code** selector in the sidebar; the choice applies to every example on the site. Where a surface deliberately lacks an operation, the tab says so and names the alternative.

The listings are extracts of example programs that CI compiles and runs against an installed package, so they cannot drift from the public interfaces. CI also checks that every public C function, C++ member function, and Python method appears in at least one of those programs. The complete quickstart programs are collected in {doc}`../../reference/examples`.

## Integration rules that matter early

A few rules prevent most architectural mistakes:

1. **PostProject identity is internal production identity.** Do not pretend a PostProject UUID is a UMID, EIDR, camera ID, or another external scheme.
2. **Preserve external identifiers exactly.** Store their scheme and value rather than translating them into the internal identity space.
3. **Never silently choose an ambiguous relink candidate.** Ambiguity is a meaningful result.
4. **Perform mutations in explicit transactions.** A transaction groups one semantic production change and its revision history.
5. **Advance a revision cursor only after a complete revision has been processed.** This keeps consumers restartable.
6. **Release owned C handles.** Treat the ownership rules in the native API as part of the contract.
7. **Do not infer relationships that were not recorded.** An activity connecting media does not automatically mean “revision,” “variant,” or “alternative.”

## Separate semantic documentation from API lookup

Use the concept pages when deciding **what an operation means**. Use generated API reference when deciding **how to call it**.

For example:

- {doc}`../concepts/assets-representations-locators` explains the representation model.
- {doc}`compound-media` shows integration workflow.
- the generated C/C++/Python reference gives exact signatures and ownership details.

This separation keeps the conceptual contract readable while allowing API reference to remain precise.

## Rust is not required for downstream integration

PostProject is implemented in Rust, but an installed native consumer should not need Cargo or Rust tooling. The public native boundary is the C ABI, with C++ and Python layers built on top of it.

If you are changing PostProject itself rather than integrating it, use {doc}`../contributors/README`.

```{toctree}
:hidden:
:caption: Getting set up

installing-a-release
c-quickstart
cpp-quickstart
python
first-production
host-object-bindings
```

```{toctree}
:hidden:
:caption: Media

media-resolution
fingerprints-and-verification
compound-media
```

```{toctree}
:hidden:
:caption: Production knowledge

external-identifiers
metadata-vocabularies
provenance
dependencies
artifacts-and-staleness
```

```{toctree}
:hidden:
:caption: Work, scale, and change

jobs-and-workers
reference-executor
bounded-queries
revision-feed
openassetio-publishing
```
