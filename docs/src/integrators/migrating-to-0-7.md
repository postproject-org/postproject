# Migrating to the 0.7 development SDK

The unpublished `0.7.0-alpha.1` SDK currently uses C ABI 41, Python `0.7.0a1`
and SQLite schema 17. Rebuild native consumers with matching headers and
library, and install a matching Python wheel. Existing production files,
UUID text, external identifiers and host binding strings retain their meaning.
Rust 1.85, C11, C++17 and Python 3.11 remain the supported floors.

## Identity values

C uses distinct `pp_production_id_t`, `pp_asset_id_t`, `pp_revision_id_t` and
`pp_transaction_id_t` structs. Asset inputs are values; arrays borrow typed
IDs and outputs write typed stack values. Use `pp_object_ref_from_asset`
for dynamic asset targets. Revision arguments are values; remove the `&`
from calls to revision-event queries and `pp_production_begin_transaction_at`.
Use typed outputs for `pp_revision_set_get`. Parse and format through the
corresponding `pp_*_id_parse` and `pp_*_id_format` functions.

C++ uses explicit `ProductionId`, `AssetId`, `RevisionId` and `TransactionId`
values. Imports return `AssetId`; use `ObjectRef::asset` for dynamic targets.
Keep IDs returned by reads in their semantic type. Explicit `asUuid()` and
byte construction are for interchange. Equality, ordering and hashing work
with standard containers. Other native identity families are still being
migrated; this development checkpoint is not the final candidate.

Python IDs are nominal `NewType` hints over ordinary `uuid.UUID` objects.
Use IDs directly; replace old `.value` access with the UUID itself. Use
`AssetRef`, `RepresentationRef`, `ResourceRef`, `ActivityRef`, `JobRef` or
`ProductionRef` for dynamic targets. Optional static checking distinguishes ID
kinds; operation checks still establish existence and production membership.

## Reads and writes

Use a {doc}`read session <coherent-reads>` when several reads inform one
decision. Copy the results and retain its detached decision base before
closing the view. Start an edit from that view or base; commit explicitly and
use its returned receipt to identify the revision it created.

Python `Transaction` and `Edit` contexts roll back uncommitted work on every
exit. Add `commit()` before leaving old transaction contexts. A failed commit
ends the edit. Re-read and reconsider
after a conflict before creating another edit. A later latest-revision query
does not identify your commit.

Discard old query cursors and restart their queries. Retained-view cursors
continue only in the same open session. Page revision events using
`revision_events_page` or the bounded native `revisionEvents` overload.
Regeneration planning reads retained facts without enqueuing work.

Metadata replacement/removal now rejects unbased transactions before staging.
Use a read session's edit or detached base; appends still merge. A destructive
edit conflicts after any intervening property change, including an append.
In Python, replace `production.transaction()` with
`production.read_session()` and `view.edit()` for removal. C++ offers
`removeMetadataProperty`. CLI removal requires `--decision-base` from
`inspect`; metadata writes return their own `commit_receipt` in JSON.
Structured conflict JSON now goes to stdout with a failing exit status;
other diagnostics remain on stderr. Update scripts that read conflicts there.

## Native options

C++ `CancelToken` and `ResolutionOptions` use fallible `create()` factories.
Check each options setter's `Result<void>` immediately; an invalid setter
preserves the previous valid settings. Keep normal Result propagation or
exception-oriented consumption according to the host's existing style.

## Job status

C++ `Job.status` is a `JobStatus` variant; use `std::get_if<JobCompletion>`
to inspect a completed result, or `stateKind()` for its category. Independent
claim, completion and diagnostic fields have been removed. The `JobState`
enum remains the category filter for job queries.

Python `Job.status` holds `JobRequested`, `JobClaim`, `JobCompletion`,
`JobFailure` or `JobCancelled`. Inspection properties (`state`, `claim`,
`completion`, `failure_diagnostic`) derive from that value. When constructing
copied job data, pass one `status` instead of the four old constructor arguments.

Job lease APIs and other state projections are still under development.
The {doc}`../../api-safety-audit` records the implementation and verification
scope of each checkpoint.
