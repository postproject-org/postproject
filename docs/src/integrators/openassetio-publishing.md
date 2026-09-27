# Publishing through OpenAssetIO

A host that already speaks [OpenAssetIO](https://openassetio.github.io/) can
publish rendered media into a PostProject production without calling
PostProject directly. The
[PostProject OpenAssetIO Manager](https://github.com/postproject-org/postproject-openassetio-manager)
is an OpenAssetIO Python manager plugin that translates OpenAssetIO's
publishing calls into PostProject transactions. This page describes what a
publish records and how other PostProject consumers see it. The mapping and
its rationale are recorded in {doc}`ADR 0010 </adr/0010-openassetio-boundary>`.

The Manager version `0.2` works with PostProject `0.4`, OpenAssetIO `1.0`, and
OpenAssetIO-MediaCreation `1.0.0-alpha.13`. Install the PostProject native
package and Python wheel, then the Manager wheel from its release page, and
initialize the Manager with the production and native library:

```python
manager.initialize({
    "production_path": "/show/production.pproj",
    "library_path": "/opt/postproject/lib/libpostproject.so",
    "root.renders": "/mnt/show/renders",
})
```

## Publish a rendered sequence

The host follows OpenAssetIO's usual flow for generating new data: ask whether
the Manager manages the trait set, `preflight` the target to get a working
reference, write the media, then `register` the finished data against the
working reference. The target is a PostProject asset or representation
reference, for example one stored as a {doc}`host-object binding
<host-object-bindings>`:

```python
from openassetio.access import PolicyAccess, PublishingAccess
from openassetio_mediacreation.specifications.twoDimensional import (
    BitmapImageResourceSequenceSpecification_v1 as SequenceSpecification,
)
from openassetio_mediacreation.traits.managementPolicy import ManagedTrait

policy = manager.managementPolicy(
    SequenceSpecification.kTraitSet, PolicyAccess.kWrite, context
)
assert ManagedTrait.isImbuedTo(policy)

render = SequenceSpecification.create()
frames = render.frameRangedTrait()
frames.setStartFrame(1001)
frames.setEndFrame(1100)
frames.setFramesPerSecond(24.0)
working = manager.preflight(
    shot_reference, render.traitsData(), PublishingAccess.kWrite, context
)

write_frames("/mnt/show/renders/sh010/sh010.####.exr")

content = render.locatableContentTrait()
content.setLocation("file:///mnt/show/renders/sh010/sh010.%7Bframe%3A04d%7D.exr")
content.setIsTemplated(True)
final = manager.register(
    working, render.traitsData(), PublishingAccess.kWrite, context
)
```

The Manager's test suite runs this flow against every PostProject revision in
CI, including an observer in a second process.

## What PostProject records

`preflight` requests a job of kind `org.postproject:openassetio-publish` for a
new representation of the target's asset; the working reference names that
job. `register` claims and completes the job in one transaction. The result is
a single revision containing:

- one new representation of the asset, with one resource and a locator, for a
  whole image sequence; a non-templated location becomes a single-file
  representation;
- the representation kind from MediaCreation's `ProxyTrait` or `OriginalTrait`,
  or *derived* when neither is present;
- the registered trait set and every trait property, stored as typed
  {doc}`metadata <metadata-vocabularies>` under the OpenAssetIO trait ID and
  property key as published;
- a producing {doc}`activity <provenance>` attributed to the host's display
  name; and
- the job's transition to succeeded, referencing that activity and
  representation.

If registration fails, nothing but the requested job exists, and the host may
register again. A preflight that is never registered leaves a requested job
that any surface can cancel through the {doc}`job protocol <jobs-and-workers>`.
Existing representations are never rewritten: publishing to a representation
reference adds a new representation of the same asset.

Resolving the final reference answers the location and frame range from the
production's current knowledge, so a relinked sequence resolves to its new
location, while every other registered property is returned as it was
published.

## Observe a publish

A publish is an ordinary committed revision. An editor or pipeline tool using
PostProject directly learns about it without rescanning by waiting on a
{doc}`revision waiter <revision-feed>` and reading the `RepresentationAdded` and
`JobSucceeded` events, from any surface and any process.

## Limitations

- Only `PublishingAccess.kWrite` is supported; `kCreateRelated` is rejected.
- Locations must be local `file://` URLs. Templates may use only the `frame`
  variable, as `{frame}` or `{frame:0Nd}`.
- The Manager does not drive trait values: `kManagerDriven` policy queries
  return no traits, so the host chooses where it writes.
- OpenAssetIO carries no source information for a publish, so the producing
  activity records no inputs, and staleness cannot be evaluated for published
  media until a host records its inputs through PostProject directly.
- The Manager is available only as an OpenAssetIO Python plugin. C, C++, and
  CLI integrations that do not use OpenAssetIO record the same facts with the
  job protocol in {doc}`jobs-and-workers`.
