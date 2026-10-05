# Coherent reads and scoped edits

Use a read session when a decision depends on several reads. Its objects,
pages, and decision base come from one retained database view. Other writers
can commit while that view stays unchanged. Close it before waiting for a UI
decision or doing expensive media work; copied values and the detached base
remain usable.

An edit started from the session retains its base automatically. A detached
base includes production identity and the observed revision, including the
initial empty journal. The store rejects another production's base and invalid
revision/sequence pairs. The base retains no database connection or lock.

```{code-variants} coherent-reads
```

Commit returns a receipt identifying this edit's production and newly created
revision. A missing revision means no new journal entry. Do not query the
latest revision to attribute a commit: another writer may already have advanced
it. Commit attempts, rollback, and disposal end the edit. Python `Edit` contexts
require explicit commit and roll back on normal exit as well as on exceptions.

The development C/C++/Python read-session surface currently provides asset and
representation point reads and bounded pages, resources, locators, logical roots, external
identifiers and known-media lookup. Rust exposes the full domain
read interface. Existing production queries read current state separately.
The CLI's `inspect --limit N` emits a bounded coherent summary and a
`--decision-base` token; a truncated summary requires a new inspection rather
than resuming a closed view.

On a semantic conflict, read fresh facts and make the decision again before
starting a new edit. See {doc}`semantic-conflicts` for conflict fields and which
independent changes can merge. Sessions use SQLite WAL for concurrent local
readers and writers. Copy or move a production only after closing its handles;
an active WAL database includes sidecar state.
