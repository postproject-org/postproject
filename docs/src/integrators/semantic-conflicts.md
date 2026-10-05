# Base revisions and semantic conflicts

Several processes can safely write one local production, but SQLite write
serialization alone cannot tell whether a host made a decision from stale
knowledge. When a write depends on previously read state, begin its transaction
with that state's revision ID.

A base revision is optimistic context, not a lock. It does not reserve the
production or reject unrelated work. Unbased transactions permit independent
metadata appends; metadata replacement/removal require a base. Other legacy
unbased mutation families remain under development review.

## Use a base for read–decide–write flows

Start with {doc}`coherent-reads`: retain one read view, copy the facts needed for
the decision, then edit from its scoped base. This also protects decisions made
from the initial empty journal. The older revision-ID recipe below remains
available during migration:

The legacy host flow is:

1. retain the latest revision before reading the relevant objects, then read
   the latest revision again and discard the reads if it changed;
2. let the application or user decide what to change;
3. begin a transaction at that revision;
4. stage and commit the change;
5. on conflict, inspect the structured detail and re-read current state; and
6. retry in a new transaction only after making the decision again.

A base is not a read snapshot. Fetching it only after reading can label an old
decision with another writer's newer revision. The two revision reads provide
a conservative fence; they do not lock the UI decision interval. Keep the
first base through that interval and use it when committing. In an empty
production there is no base: an independent first import can use an ordinary
transaction. A decision requiring existing committed objects must first have
those objects and a retained revision.

Commit receipts identify the production and the revision created by that
transaction. An absent revision explicitly means no new journal entry. C uses
`pp_transaction_commit_with_receipt`, C++ uses `commitWithReceipt`, Rust uses
`commit_with_receipt`, and Python `commit()` returns the receipt. The legacy
void commit operations remain available during migration. A later head read
can belong to another writer; use the atomic receipt for attribution.

```mermaid
sequenceDiagram
    participant A as Host A
    participant P as Local production
    participant B as Host B
    A->>P: Read fact K at revision R
    B->>P: Read fact K at revision R
    A->>P: Commit K with base R
    P-->>A: Success at revision S
    B->>P: Commit K with base R
    P-->>B: Conflict: key K, base R, superseding S
    B->>P: Re-read K and revision S
    B->>P: Explicit retry with base S
    P-->>B: Success or a new conflict
```

The example deliberately makes one writer stale, inspects the semantic key and
both revisions, then performs an explicit retry:

```{code-variants} semantic-conflicts
```

For the CLI, use `--decision-base TOKEN` from `inspect`. The advanced
`--base-revision REVISION_ID` path remains available. With `--json`, an
optimistic conflict is written to standard output with a failing exit status as
JSON containing `key`, `base_revision_id`, `base_revision_sequence`,
`superseding_revision_id`, and `superseding_revision_sequence`.

## Conflicts follow semantic facts

PostProject does not compare only the production's latest revision. It checks
the non-mergeable facts touched by the transaction:

- the locator set of one resource;
- one metadata property on one object;
- the dependency set of one representation;
- one media root;
- one exact external-identifier attachment; and
- one resource or representation fingerprint algorithm/version domain.

Creating objects with new identities and appending independent facts can still
commit from an older base. For example, two hosts may add distinct assets or
media roots from the same base. Replacing the same locator set or changing the
same media root from that base produces exactly one successful update and one
conflict.

The complete per-operation classification is recorded in {doc}`ADR 0042
</adr/0042-semantic-write-conflicts>`. Job transitions retain their more
specific state, lease, and claim-token guards rather than becoming generic
optimistic conflicts.

Metadata appends advance the property version while remaining mergeable.
Replacement/removal guard that version, so an intervening append invalidates
a destructive decision (ADR 0052). Rejection rolls back the whole edit.

## Treat the result as a new decision point

Conflict messages are diagnostic and may change. Branch on the stable error
category and structured conflict fields instead. The key says which fact
changed, while the superseding revision can be loaded or presented alongside
its revision origin.

A failed conflict commit changes nothing and closes that transaction. Do not
retry it automatically: the new value may represent another application's or
person's intentional decision. Re-read through the production handle, update
the host UI or policy input, and create a new transaction with a refreshed
base if the change is still wanted.

Set an `OriginIdentity` and useful message on every host-authored transaction.
Origin identifies the integrating application and optional version/URI; it is
not an authenticated user identity. It gives revision and conflict UI useful
context about which tool made the superseding change.

## Local shared-production boundary

The supported shared-write shape is multiple processes on one machine opening
the same local `.pproj` file. The revision waiter lets each process notice the
other's commits, and base revisions protect read–decide–write operations from
silent lost updates.

This contract does not cover network filesystems, remote services, access
control, distributed ordering, CRDTs, or automatic merging. Do not treat a
`.pproj` file as a network database.
