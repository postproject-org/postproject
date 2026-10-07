# 0055: CLI dependency submission outcomes

Status: accepted for development.

## Decision

`dependency record` reports the accepted ordered submission, whether it changed
the observation, and the edit's atomic commit receipt. Construct the response
before commit from the validated request and staging result. Emit it only after
successful commit. Do not reload live facts after commit: another writer could
replace the set before that read, mixing its observation with our receipt.

An unchanged submission returns `changed: false` and a no-change receipt.
It does not claim a new observation revision. `dependency show` remains the
explicit read of stored status, observation revision and edges; its facts may
have changed since the submission. Human output reports the source, outcome
and accepted edge count.

## Migration and standards impact

In development CLI JSON, `dependency record` replaces `status` and
`recorded_at_revision` with `changed`. Use `commit_receipt.revision.sequence`
for a changed submission's revision, or `dependency show` for current facts.
No Rust/C/C++/Python operation, ABI layout, schema or persisted UUID changes.

Reviewed against the standards policy: dependency kinds and authored references
retain their exact meanings and order; no external standard mapping changes.
