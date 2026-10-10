# Exchange portable production knowledge

The development SDK can export a consistent checkpoint, create a passive mirror
and catch it up with complete committed records. These operations are
experimental; the released 0.7 SDK does not provide them. One authority owns the
history. A mirror supports ordinary reads and local media resolution with the
recipient's root mappings; it rejects edits and worker authority.

Export a checkpoint and import it into a new destination. Validation uses the
same complete domain checks in temporary private storage. Existing destinations
and incomplete streams reject without publishing a partial mirror.

````{code-variants} exchange-checkpoint
```{no-variant} c
The native exchange projection is not yet available. Use the development CLI.
```
```{no-variant} cpp
The C++ exchange projection is not yet available. Use the development CLI.
```
```{no-variant} python
The Python exchange projection is not yet available. Use the development CLI.
```
````

A proposal has stable client/request identities and ordered semantic commands.
Keep those identities and prepared new-object IDs when retrying. Recover a lost
reply by lookup or by submitting the same intent again. The returned receipt
identifies that request's own revision; an accepted no-op has no new revision.
The CLI example submits a no-op; the Rust example appends an exact integer.
Changed intent needs a new request. Destructive operations need a scoped decision
base; a stale conflict requires a fresh read and a new decision.

````{code-variants} exchange-submit
```{no-variant} c
The native proposal builder is not yet available. Use the development CLI.
```
```{no-variant} cpp
The C++ proposal builder is not yet available. Use the development CLI.
```
```{no-variant} python
The Python proposal builder is not yet available. Use the development CLI.
```
````

Catch-up exports every complete record after a saved source position through one
pinned head. Ordinary native transactions enter that history automatically.
Positions retain production, history, revision, sequence and digest. Observation
feed cursors and connection-local query cursors cannot substitute for them.

````{code-variants} exchange-catch-up
```{no-variant} c
The native catch-up projection is not yet available. Use the development CLI.
```
```{no-variant} cpp
The C++ catch-up projection is not yet available. Use the development CLI.
```
```{no-variant} python
The Python catch-up projection is not yet available. Use the development CLI.
```
````

Each applied record is atomic. If later input fails or the process stops, earlier
complete records remain durable. Reopen, inspect the mirror head and retry the
same stream; verified duplicates create no revision. A missing predecessor or
pre-floor position reports `history_gap` and requires a checkpoint. Unsupported
required features reject; old histories are never guessed or merged.

Files carry bounded chunks, not media bytes. Defaults allow 1 GiB encoded input;
checkpoint import also allows 2 GiB private database/journal space and 10 million
frames/traversal operations. Raise receiver budgets explicitly for larger
productions. Rust writers return their final seal after streaming; publish output
only after success and durable flush. CLI exports publish new files exclusively.
Long pinned exports can retain SQLite WAL pages.

Job observations retain attribution, lifecycle and publication evidence without
usable credentials. Worker submissions use private lease handles or token files;
see {doc}`../reference/cli-output` for explicit CLI delivery and recovery.
Duplicate lookup never reissues ownership. Lost credential delivery needs expiry
or coordinator cancellation.

Hashes detect inconsistency relative to the saved source anchor; they do not
authenticate a source or prevent two copied authorities from diverging. No
automatic pruning, media transfer, network service, editable replica or authority
restore is provided. A checkpoint cannot restore private outcomes, clocks or
worker capabilities.
