# ADR 0010: OpenAssetIO boundary

- Status: Accepted
- Date: 2026-09-20

## Context

Applications that need an interchangeable asset manager already have an
industry host-to-manager contract. A second PostProject-specific host protocol
would increase integration cost and couple applications to one implementation.
At the same time, PostProject has embedded and specialized capabilities that do
not belong in a general manager interface.

## Decision

OpenAssetIO is the preferred interoperability contract between a host
application and an asset manager. PostProject will be able to act as a manager
through a future `postproject-openassetio` adapter.

The adapter will translate:

- opaque entity references to stable PostProject objects;
- locatable content to representations, resources, and locators;
- image-collection traits to compact image-sequence structures;
- manager resolution requests to PostProject resolution results; and
- publishing operations to PostProject transactions, provenance, and revisions.

OpenAssetIO and OpenAssetIO-MediaCreation types do not enter
`postproject-core`. Their evolving traits and version-specific behavior remain
at the adapter boundary. The adapter is the separately maintained
`postproject-openassetio-manager`, which uses only PostProject's public Python
API; a capability it needs is added to that API in domain terms first.

### Publishing

Publishing follows the OpenAssetIO 1.0 contract (`preflight`, `register`, and
working references, as documented in the pinned `v1.0.2` source) and maps onto
the job protocol of ADR 0023, so a publish is ordinary production work:

- `preflight` on an asset or representation requests a job of kind
  `org.postproject:openassetio-publish` for a new representation of that asset.
  The working reference is the job's host-object binding. A representation is
  never rewritten in place; writing to one adds a representation of its asset.
- `register` claims and completes that job in one transaction containing the
  representation, its resources and locator, the registered trait data, and
  the producing activity. `register` without `preflight` also requests the job
  in that transaction. Either way the publish appears as one revision or not
  at all.
- A templated MediaCreation location with a `{frame}` variable and
  `FrameRangedTrait` is recorded as one image-sequence representation.
- OpenAssetIO requires a manager to persist the registered trait set and the
  properties of traits it manages. The trait set is stored as metadata
  `https://postproject.org/ns/openassetio/1` `traits`, and every property that
  PostProject does not record as structure under its trait ID and property key
  exactly as published. Location and frame structure are answered from current
  production knowledge, so relinked media resolves to its new location.

Preflight requests the job rather than claiming it. A claim needs a lease the
adapter cannot size for an unknown render, and its token is a capability:
OpenAssetIO hands working references to other processes, and a reference must
not carry a credential. Registration therefore takes a short claim and
completes it in the same commit. A separate publishing reservation outside the
job model was rejected because it would duplicate the lifecycle, revision
events, and cancellation the job protocol already provides. OpenAssetIO carries
no source information for a publish, so a published representation records its
producing activity without inputs.

PostProject retains its native API for embedded use and for specialized
capabilities such as fingerprint evidence, provenance traversal, and revision
feeds.

## Standards impact

The mapping follows OpenAssetIO 1.0.2 and OpenAssetIO-MediaCreation
1.0.0-alpha.13. Trait IDs and property keys are stored verbatim and never
rewritten beneath `postproject.org`; the only identifier the adapter mints is
the `https://postproject.org/ns/openassetio/1` vocabulary for the trait set,
under ADR 0013. Image sequences follow the MediaCreation
`BitmapImageResourceSequence` specification: a percent-encoded templated
location with `isTemplated` set, `FrameRangedTrait`, and `ImageCollectionTrait`.
A floating-point `framesPerSecond` is stored as an exact rational rate,
recognizing the 1000/1001 family; OpenAssetIO defines no rational rate.
Relationship queries and the `kCreateRelated` access mode are not mapped.

## Consequences

A host can support PostProject without hard-coding it as the only asset manager,
and PostProject can evolve its internal persistence independently of an external
contract. Mapping tests must demonstrate that one image sequence remains one
representation and that locatable content does not collapse resources into
representations. Any PostProject consumer observes a publish through the
revision feed, and an abandoned preflight is visible as a requested job that
any surface can cancel.
