# Artifacts and staleness

A proxy, thumbnail, transcode, or render that PostProject knows was produced by
a recorded activity is a *managed artifact*. PostProject can tell a host
whether that artifact still reflects its inputs, explain the answer, and say
whether the recorded knowledge is enough to reproduce it. The model is
described in [artifact knowledge and
reproducibility](../concepts/artifact-knowledge.md); this guide shows the
workflow.

Evaluation reads recorded knowledge only. It never opens media files, so it is
cheap enough for a UI and gives the same answer on every machine. Whether the
artifact's files are *reachable* is a separate question answered by
[resolution](media-resolution.md).

## Evaluate an artifact

An evaluation returns one knowledge state — current, stale, indeterminate, or
diverged — with structured reasons for anything that is not current, and a
separate reproducibility report listing each missing condition:

```{code-variants} artifact-knowledge
```

Traversal through upstream artifacts is bounded by depth and by the number of
representations visited. A bound that is reached is reported as a reason and
makes the result indeterminate, never current.

## After a source changes

Staleness starts with a recorded change. When a camera original is replaced in
place, the host records the new [fingerprint
observation](fingerprints-and-verification.md#record-a-new-fingerprint-observation);
from then on every artifact produced from the old content evaluates as stale,
with a reason naming the input edge, the fingerprint domain, the snapshot, and
the current value:

```{code-variants} stale-after-change
```

A reason chain also covers artifacts produced from other artifacts, such as a
render made from a stale proxy, and changes that arrive through a required
[dependency](dependencies.md).

Evaluation never reads files, so it cannot notice a source that someone else
replaced, for example graded media copied over the original by another
application. When a source may have changed outside the host, observe it first
and then evaluate. The observation reports whether the content changed, and
records nothing when it did not. A worker also observes its inputs before it
claims a job, so the activity's snapshots describe the content it actually
read.

## Find and regenerate stale artifacts

To list every stale artifact, optionally only downstream of one source, page
through the stale-artifact query described in [bounded
queries](bounded-queries.md#find-stale-artifacts). To regenerate one, derive the
job that would reproduce it from its producing activity and enqueue it
explicitly; see [plan and enqueue
regeneration](jobs-and-workers.md#plan-and-explicitly-enqueue-regeneration).
Nothing is regenerated automatically.
