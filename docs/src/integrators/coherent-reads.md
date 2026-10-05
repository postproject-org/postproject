# Coherent reads and scoped edits

Use a read session when a decision depends on several reads. Its objects,
pages, and decision base come from one retained database view. Other writers
can commit while that view stays unchanged. Close it before waiting for a UI
decision or doing expensive media work; copied values and the detached base
remain usable.
Resolution and verification use that view's stored knowledge while inspecting
the current filesystem. A read session does not freeze paths or media bytes.

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
representation point reads and bounded pages, resources, locators, metadata,
logical roots, external identifiers, known-media lookup, jobs, artifact reports,
resolution and verification. Rust exposes the full domain read interface. Existing production queries read current state separately.
The CLI's `inspect --limit N` emits a bounded coherent summary and a
`--decision-base` token; a truncated summary requires a new inspection rather
than resuming a closed view.

On a semantic conflict, read fresh facts and make the decision again before
starting a new edit. See {doc}`semantic-conflicts` for conflict fields and which
independent changes can merge. Sessions use SQLite WAL for concurrent local
readers and writers. Copy or move a production only after closing its handles;
an active WAL database includes sidecar state.

To retain a decision across processes, export its base as a canonical token:
C uses `pp_decision_base_format`/`pp_decision_base_parse`, C++ uses
`DecisionBase::toToken`/`fromToken`, and Python uses `to_token`/`from_token`.
The CLI accepts the same token with `--decision-base`. A token retains neither
a view nor a lock; beginning an edit checks its production and revision.
