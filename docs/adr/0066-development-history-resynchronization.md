# 0066: Preserve incomplete development history during resynchronization

Status: accepted

Early development builds captured partial effects and could leave gaps between
complete records. Those bytes are evidence, not a complete replay prefix. A
changing commit must continue to reject an unavailable predecessor.

An explicit authority operation takes a fresh scoped head base. When the retained
record chain is incomplete, it establishes a deterministic checkpoint floor at
that head in one writer transaction. It preserves the history generation,
original observations, earlier record manifests/chunks, partial effects, prior
anchor and private submission outcomes. Complete chains keep their existing
floor. Reopening or ordinary commits never perform this recovery automatically.
Stale or foreign bases, mirrors and pinned views reject the operation.

Checkpoint archives carry earlier evidence separately from the contiguous suffix.
Bounded typed archive frames preserve exact bytes and original boundaries. They
are never executed, interpreted as SQL or advertised as complete records. Archive
items must reference retained revisions at or below the floor. Private outcomes,
credential bindings, worker secrets and authority clocks remain excluded.

Existing mirrors behind the new floor must import a new checkpoint. This neither
promotes a mirror nor restores lost worker authority. The operation cannot recover
missing bytes or establish authenticity; diagnostics retain that limitation.

Standards impact: this defines local PostProject history behavior. It changes no
external vocabulary, identifier normalization or standards mapping.
