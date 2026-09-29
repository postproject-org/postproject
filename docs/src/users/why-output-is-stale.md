# Why is this output marked stale?

A stale output is a derived representation—such as a proxy, thumbnail, render,
or transcode—that was made from input knowledge which is no longer current.
PostProject does not decide that by comparing file dates. It compares what was
known when the output was made with what the production knows now.

## The short version

When an application records how it made an output, it can snapshot the observed
content of each input. Later, if an input's effective recorded fingerprint has
changed, PostProject reports the output as **stale** and identifies the changed
input. A host can then offer to rebuild it, keep it, inspect the difference, or
do nothing.

PostProject never rebuilds an output merely because it is stale. Rebuilding is
an explicit host or worker action.

## Observed content, not just a path

A locator says where bytes were observed. A fingerprint observation says which
bytes were observed there in a particular fingerprint domain. These facts are
separate because a path can stay the same while its file is replaced, or a file
can move without changing its content.

An output activity may snapshot an input representation's effective
fingerprints. If the source is later verified and a new fingerprint becomes
effective, the old snapshot no longer describes the input. That is a direct
staleness reason.

Hosts should verify or observe changed source content before asking for a stale
evaluation. A filesystem modification time alone is not durable production
knowledge.

## Dependencies can make an output stale indirectly

A representation can record dependencies on assets or other representations.
For example, a shot render may depend on a character layer, which in turn uses
a texture. If the texture changes, the render can become stale through that
dependency path even though the shot's own source file did not change.

The evaluation result carries the dependency path and the affected object. A
host should show that reason instead of only saying “out of date.” The
dependency object itself is not automatically stale: staleness applies to the
derived output whose recorded inputs no longer match.

## Current, stale, and unknown are different

**Current** means the recorded snapshots and dependency knowledge still agree
with the production's effective content evidence.

**Stale** means PostProject has positive evidence of a relevant change, such as
a different effective input fingerprint or changed dependency path.

**Unknown** means the production lacks enough evidence to decide. Missing
fingerprints, incomplete dependency knowledge, and a bounded traversal that
reaches its limit are examples. Unknown is not silently treated as current.

## Current does not always mean reproducible

An application may discover an existing output after it was made and record it
without having observed the worker, tool version, arguments, or complete input
set. The output can still be **current** with respect to the dependency evidence
that was recorded. It is **current but not reproducible**: PostProject cannot
describe a complete recipe that another worker could reliably repeat.

This commonly happens when a host begins tracking an existing proxy or records
a render only after the host reports completion. A good UI should say both
things: the known inputs have not changed, and the production does not contain
a complete recipe.

## What an application may do next

For a stale output, a host may:

- show the changed input and dependency path;
- request a regeneration job using the previous output's known job kind and
  target root;
- let a worker claim and complete that job;
- record the replacement output and new input snapshots;
- preserve the old output as historical knowledge if its locator remains
  meaningful.

For an unknown result, the host may collect the missing content or dependency
evidence and evaluate again. For a current-but-not-reproducible output, it may
keep using the output, or deliberately rebuild it through a fully observed
worker to establish reproducible provenance.

Hosts remain responsible for policy. PostProject reports recorded knowledge
and reasons; it does not delete files, silently select a replacement, or start
work on its own.
