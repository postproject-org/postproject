# Jobs and workers

PostProject persists the production meaning of requested work. Your host,
service, or script remains responsible for choosing jobs, starting processes,
managing resources, and applying retry policy.

## Request work

Create the job and any parameter metadata in one transaction. The request names
its input representations and the desired output asset and representation kind,
optionally with a logical target root for the output. Parameters use the normal
typed metadata model with the job as their target:

```{code-variants} request-job
```

Listing jobs returns a bounded page in stable identity order. State and
open-world kind are optional exact filters. Pass the opaque `next_cursor` back
with the same filters and page size to continue; pages are weakly consistent
across commits:

```{code-variants} job-query-pages
```

## Claim, renew, and release

A worker claims a requested job with tool attribution, optional agent
attribution, the caller-supplied current time, and a lease expiry. The claim
returns a token. Keep it private: renew, release, complete, and fail all
require it, so two processes with the same identity cannot finish each other's
claims. The token is a production-scoped capability, not an authentication
credential.

Renew the claim before the lease expires when work takes longer than one lease.
Release it to give the job back without recording a failure:

```{code-variants} claim-job
```

Two processes cannot hold a valid claim on the same job. An expired lease is
claimable again when a later claimant supplies a current time at or beyond the
expiry. Every token-checked transition rejects a stale or unrelated token. Time
is always supplied by the caller; storage never reads the clock for leases.

## Complete a job

On success, stage the output representation and the activity that produced it,
then complete the job in that same transaction. The output, the activity with
its input snapshots, and the job's transition to succeeded become durable
together or not at all:

```{code-variants} complete-job
```

Completion validates that the activity consumes the job inputs and produces
the staged output requested by the job. Do not commit the representation or
activity in an earlier transaction: only `complete` gives the all-or-nothing
guarantee. Library callers can stage any representation structure — single
resource, image sequence, ordered parts, or package — before completing; the
CLI completion adapter creates a single-file output.

Staging an output representation fingerprints its files, so a new output needs
no separate fingerprint step. A worker that instead rewrites existing media in
place must observe that content in the transaction:

- a proxy regenerated at its old path;
- a source replaced by a conform.

See [recording a new fingerprint observation](fingerprints-and-verification.md#record-a-new-fingerprint-observation).
Observation records the new resource value and recomputes every representation
that uses the resource, so [artifact evaluation](artifacts-and-staleness.md)
reports the artifacts that depend on it as stale. Never write a representation
fingerprint of your own devising. Only PostProject's own values take part in
staleness and resolution.

## Fail or cancel a job

On a tool error, fail the job with a bounded diagnostic. A failed job never
leaves a representation or activity behind:

```{code-variants} fail-job
```

Cancelling is administrative and needs no claim token. It is permitted while
the job is requested or claimed:

```{code-variants} cancel-job
```

## Learn when work finishes

A requester does not need to poll the job list. Every job transition is a
semantic revision event, so wait on a [revision waiter](revision-feed.md) and
fetch the revisions filtered to `JobSucceeded`, `JobFailed`, and `JobCancelled`
events after your cursor. This observes a worker in another process — for
example `postproject job run` — as soon as it commits.

## Public operations

| Operation | Rust | C | C++ | Python | CLI |
|---|---|---|---|---|---|
| request | `request_job` | `pp_transaction_request_job` | `requestJob` | `request_job` | `job request` |
| list | `jobs` | `pp_production_jobs` | `jobs` | `jobs` | `job list` |
| read one | `job` | `pp_production_job` | `job` | `job` | `job show` |
| claim | `claim_job` | `pp_transaction_claim_job` | `claimJob` | `claim_job` | `job claim` |
| renew | `renew_job_claim` | `pp_transaction_renew_job_claim` | `renewJobClaim` | `renew_job_claim` | `job renew` |
| release | `release_job_claim` | `pp_transaction_release_job_claim` | `releaseJobClaim` | `release_job_claim` | `job release` |
| complete | `complete_job` | `pp_transaction_complete_job` | `completeJob` | `complete_job` | `job complete` |
| fail | `fail_job` | `pp_transaction_fail_job` | `failJob` | `fail_job` | `job fail` |
| cancel | `cancel_job` | `pp_transaction_cancel_job` | `cancelJob` | `cancel_job` | `job cancel` |
| plan regeneration | `plan_regeneration` | `pp_production_plan_regeneration` | `planRegeneration` | `plan_regeneration` | `job plan` |

The C ABI returns owned job and regeneration-plan sets; reading one job returns
a one-element job set. A job page's optional
cursor borrows the job set and must be copied before release. Release every
returned set with its matching release function. The C++ and Python bindings
copy result values and cursors into their native immutable types. The CLI
accepts `job list --state`, `--kind`, `--limit`, and `--cursor`; JSON output is
a page object with `items` and `next_cursor`.

The opt-in [reference local executor](reference-executor.md) builds on this
protocol for proxy and thumbnail jobs. Its subprocess adapter is available in
Rust and its complete runner is available as `job run`; C, C++, and Python
workers use the protocol above rather than an executor wrapper.

## Plan and explicitly enqueue regeneration

`plan_regeneration` accepts artifact representation IDs and returns proposals
derived from recorded producers. Each proposal includes a fresh requested job
and parameter metadata already retargeted to that job ID. Planning requires one
unambiguous producer for each artifact.

The proposal is not durable. To enqueue it, open a transaction, request the
proposed job, copy its parameter assertions to the job target, and commit. This
separation lets a host review, prioritize, modify, or discard proposed work and
prevents a read from creating background work.

```{code-variants} plan-regeneration
```

See [jobs and production work](../concepts/jobs.md) for the domain model and
[revision feed](revision-feed.md) for cross-process change discovery.
