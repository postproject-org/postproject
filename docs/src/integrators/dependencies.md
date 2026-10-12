# Dependency integration

Many production files reference other production objects: a USD shot layer
references a character layer, a Nuke script reads plates, a timeline references
clips, a material references textures. PostProject records those references as
[dependency relationships](../concepts/dependencies.md). It never parses a file
to find them: the host, or an adapter that understands the format, extracts the
references and records them.

## Record a complete dependency set

Dependencies are an observation of one representation's content, so a host
records the *complete* set each time it extracts references, not individual
edges. Each dependency has an open-world kind, a target — an asset whose
current representation is used, or one pinned representation — a required or
optional flag, and the authored reference exactly as it appears in the file.

After a new fingerprint observation of the source, its recorded set is marked
as needing extraction: the old set is kept, but no longer presented as current
knowledge until the host records a fresh one.

```{code-variants} dependency-set
```

Recording an identical set is a no-op. An explicitly recorded empty set means
"inspected, no references", which is different from never having recorded a
set.

The {download}`runnable scene/texture fixture <../../examples/python/authored_dependencies.py>`
extracts the references actually present in a small JSON scene. The texture
declares an empty reference array, so its empty observation is justified. A
render records the scene as a provenance input; changing the referenced
texture makes that render stale without inventing a dependency on the render.

## Query dependencies and dependents

Forward queries follow a representation's dependencies; reverse queries answer
"which shots use this character?". Both are bounded by depth and by the number
of representations visited, paged with an opaque cursor, and report separately
when a bound truncated the traversal:

```{code-variants} dependency-queries
```

Cycles are permitted, because composition systems do not guarantee acyclic
references. The bounds make every traversal finish regardless. See [bounded
queries](bounded-queries.md#traverse-dependencies) for the paging contract.

## Dependencies and staleness

A source whose dependency changed is not itself stale: it picks up the change
the next time it is opened. Artifacts *produced from* that source are stale,
because the activity captured the dependency closure when it ran. [Artifacts
and staleness](artifacts-and-staleness.md) shows the evaluation.
