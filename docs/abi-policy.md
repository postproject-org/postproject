# ABI policy

ABI version 51 is pre-release and may change during the 0.x series, with every
change recorded in the changelog and ABI tests. `pp_abi_version()` reports the
implemented version. Exported symbol names are unversioned until the first stable
release, but removals or signature changes require an explicit ABI-version bump.

The package is `0.7.0-alpha.1` (Python `0.7.0a1`). Its APIs remain experimental
after the coordinated migration and compatibility review; no additional stable
family is named. The released 0.6 promise below remains intact.

Current development migrates local files to schema 21 for persistent exchange
history/floor identity (ADR 0065). The released 0.7 baseline remains schema 19.
Exchange implementation and projections are incomplete; this is not a new
package release or a stable protocol/ABI promise.

Release 0.6 names the C++17 `cpp-result-propagation` family (ADR 0020).
`Result<T>` (including `Result<void>`), `POSTPROJECT_TRY` and
`POSTPROJECT_TRY_ASSIGN` retain their success/error ownership and propagation
protocol within `0.6.x`. Diagnostic text and compiler-specific layouts are
not promised. Other APIs remain experimental. This names no C ABI operation family or cross-series
binary replacement promise.

Release 0.5 named no compatibility subset (ADR 0020). Every API in the `0.5.x`
series remains experimental and may change within the series. The mechanically
eligible external-identifier and resolution families are not promised in
isolation because their production and transaction lifecycle dependencies lack
the required complete evidence from two independent hosts (ADR 0040).

## Types and ownership

The 0.7 candidate uses distinct native IDs, checked content/state payloads,
coherent read sessions, production-scoped decision bases and atomic commit
receipts. Use the receipt-returning commit for ordinary writes. Every commit
attempt is terminal; no-change commits create no revision. Read results own
their copied facts and survive session release. Filesystem verification observes
current bytes against the retained database knowledge.

C payload accessors require the selected kind/state and clear outputs on a
mismatch. C++ and Python own checked alternatives. Unknown closed tags are
unsupported; malformed payloads are corruption (ADRs 0061–0064).

Job leases own production-bound worker authority. Durations are exact whole
microseconds from 1 us through 24 h. Claiming edits activate pending ownership
only on commit; transitions and publication recheck the authority clock and
current state. Freeing a handle never releases a durable claim. Public job facts
contain attribution and expiry, with private credential transport kept separate
(ADRs 0057–0058).

Schema 19 preserves stored identities, media knowledge and revisions. Schema 18
adds journaled file-fact observations; schema 19 expires legacy claims while
introducing authority time. Explicit observations and destructive edits require
decision bases. These internal contracts introduce no standards mapping.

Productions, transactions, asset sets, media-root sets, representation sets,
resolution sets, activity sets, external-identifier sets, object-reference sets,
object-query sets, locator-query sets, dependency sets, dependency-query sets,
job sets, regeneration-plan sets, metadata sets, metadata values, metadata
inputs, artifact evaluations, reproducibility reports, revision sets,
revision-event sets, revision waiters, fingerprints, cancellation tokens,
resolution options, media sources, known-media sets, read sessions, job leases, and errors are opaque handles. A
successful creation/open call transfers one production ownership reference to the
caller, which releases it exactly once with `pp_production_release`. Failed calls
optionally transfer an error object, released exactly once with
`pp_error_release`. Release functions accept null as a no-op; releasing the same
non-null pointer twice is invalid.

A production permits one open transaction at a time. Import and media-root mutations
are prepared and staged in memory, then persisted together by
`pp_transaction_commit`. Rollback or release of an open transaction discards all
staged work. A transaction retains the underlying production state, so its handle
remains valid if the originating production handle is released. Closed transaction
handles may only be released.

`pp_uuid_t` contains exactly 16 network-order UUID bytes. `pp_object_ref_t`
combines that ID with a fixed-width object-kind tag; open-world concepts such as
identifier schemes remain UTF-8 strings rather than C enums. Numeric errors and
object kinds are fixed-width values defined in the C header.

## Strings and errors

Input strings are borrowed, NUL-terminated UTF-8 and may not contain embedded NUL.
Optional strings use null. Error messages are borrowed NUL-terminated UTF-8 owned
by their error object and remain valid until that object is released. Stable error
codes are the contract; message wording is diagnostic and may evolve.

## Panics and threading

Every exported operation contains Rust unwinding with `catch_unwind`. Panics are
translated to `PP_ERROR_INTERNAL`; no panic may cross the C boundary. Production
handles may move between threads and support concurrent calls. Calls on one handle
serialize internally and block rather than reporting a contention conflict. A
panic while the handle is locked does not poison later calls.

Transaction, result-set, and error handles require caller-side serialization. No
handle may be released while another thread uses it. Transactions stage mutations
without holding the production lock; commit serializes with operations using the
same production state. Opening the production again provides a separate handle for
reads during that interval, subject to SQLite's own file-locking behavior.

A revision waiter is created from a production but owns its own read connection
and never uses the production handle again, so a blocked wait holds no
production lock. Waiters are caller-serialized, except that
`pp_revision_waiter_cancel` may be called from any thread while another thread
waits; a concurrent second wait on one waiter returns `PP_ERROR_CONFLICT`.
Releasing the production closes its waiters, whose current and later waits
return `PP_REVISION_WAIT_CLOSED`. The ABI never calls back into foreign code.

## Header compatibility

The hand-reviewed C header is authoritative. Rust implementation types, SQLite
types, allocation APIs, and standard-library layouts never cross the ABI.

## Opt-in usage evidence

When `POSTPROJECT_ABI_TRACE` names an output file, every exported operation
reached in that process is recorded once as a sorted C symbol name. Tracing is
disabled by default, records no arguments or production data, and never changes
an ABI result when evidence cannot be written. Concurrent processes use
separate files. The compatibility report combines those explicit traces with
the operation families in `docs/compatibility-families.toml`; neither the trace
nor the generated report is itself a compatibility promise (ADR 0040).

## C++ wrapper

`postproject.hpp` is a header-only C++17 wrapper over the authoritative C API.
It owns production and transaction handles with RAII and makes both wrappers
move-only. Destruction of an open transaction invokes the C release behavior
and therefore discards staged work.

Every fallible operation returns `postproject::Result<T>`, holding the value or
a `postproject::Error` value. The error keeps the stable `ErrorCode` and copies
the diagnostic text before the C error object is released. The header contains
no `throw` outside `Result::value()`, and that throw is compiled only when
exceptions are enabled. Without exceptions, `value()` on an error aborts. The
header therefore compiles with `-fno-exceptions` (ADR 0032). Inputs containing
embedded NUL bytes return `ErrorCode::invalid_argument` before C is called.

`Result` uses the member names of C++23 `std::expected<T, Error>`, including
`and_then`, `transform`, and `or_else`. `POSTPROJECT_TRY` and
`POSTPROJECT_TRY_ASSIGN` are public and stay defined after the header; they
return a failed `Result`'s error from the enclosing function (ADR 0033).

The wrapper adds no domain behavior and exposes no C++ standard-library type
through exported library symbols. The Result propagation protocol has the
source compatibility promise stated above for `0.6.x`; other wrapper operations
follow the pre-release policy independently of the C ABI version.

## Resolution results

`pp_production_resolve_assets` returns an immutable opaque set containing one
result per representation of each requested asset, in asset order. Each representation reports aggregate availability,
ordered resource results, and availability issues such as offline required
resources or missing sequence frames. Fixed-width states, issue kinds, frames,
candidates, and evidence are read through index-checked accessors. Candidate URI
and optional evidence-detail strings are borrowed from the result set and remain
valid until `pp_resolution_set_release`. The C++ wrapper copies these into
`RepresentationResolution`, `ResourceResolution`, `AvailabilityIssue`,
`ResolutionCandidate`, and `Evidence` values, so their lifetime is independent
of the C handle.

Resolution never mutates a production. A caller explicitly stages a selected
candidate using `pp_transaction_confirm_locator`, and only transaction commit
makes that location durable. The caller is responsible for passing a URI from
the result it reviewed; the API validates the URI and resource identity at
persistence time but does not silently choose a candidate.

Resolution snapshots the database state it needs while holding the production
lock, then releases that lock before filesystem discovery and fingerprinting.

Logical media-root names are production knowledge. Root mappings and search
directories in `pp_resolution_options_t` are machine-local; the library copies
and validates them when they are added and never records them. A null options
pointer means the defaults, and a null asset array is valid only with a zero
count. Cancellation tokens may be cancelled from any thread. Root summaries expose an optional
legacy absolute URI solely for lossless migration from schema versions before 6.

## Representation inspection

`pp_production_representations` returns immutable snapshots of an asset's
representations. Index-checked accessors expose structure kind, ordered members,
requiredness and roles, compact image-sequence descriptors, concrete resources,
locators, and their last observed availability. Resource fingerprints and
structure-aware representation fingerprints have separate accessors and counts;
callers must not treat one as the other. Returned strings and fingerprint byte
spans borrow the result set and remain valid until
`pp_representation_set_release`.

The C++ wrapper copies the complete snapshot into `Representation`, `Resource`,
`Locator`, and `Fingerprint` values. Compact image sequences remain one resource
with a frame domain rather than one synthetic resource per frame; each of its
locators carries the naming of its files.

ABI version 16 adds explicit transaction mutations for resource and
structure-aware representation fingerprint observations. Callers retain their
input buffers; the transaction copies them and persists them only at commit.
Activity-edge accessors also expose storage-captured snapshot revisions and
fingerprints. Their returned strings and byte spans borrow the activity set;
legacy migrated edges report an absent snapshot explicitly.

ABI version 17 adds owned artifact-evaluation and reproducibility handles.
Their reason and issue records are fixed-layout borrowed views; strings and
fingerprint byte spans remain valid until the owning handle is released.
Evaluation is knowledge-only and uses caller-supplied traversal bounds.

ABI version 18 extends artifact reasons with typed dependency paths and
dependency-specific incomplete-knowledge conditions. Path arrays, their
strings, and fingerprint spans borrow the owning evaluation handle. It also
adds owned direct-dependency reads, reverse dependent reads, complete-set
transaction recording, and the dependency-set-recorded revision event.

ABI version 19 adds the job object-reference kind and projects all seven job
lifecycle revision events. Job events carry only the job ID; claim-token
capabilities are never published through the revision feed.

ABI version 20 adds owned job-set reads, complete state-specific job views,
indexed input access, and transaction-staged job requests. Strings and job
views borrow the result set; request inputs are copied into the transaction.

ABI version 21 adds transaction-staged job claim, lease renewal, release,
failure, and administrative cancellation. Claim returns a random capability
token before commit so a caller can retain it, but the token becomes usable
only after the transaction commits successfully. Lease time remains explicitly
caller-supplied.

ABI version 22 adds atomic job completion by binding a representation and
activity already staged in the same transaction. This reuses every public
representation shape while ensuring the output, its resources, provenance
snapshots, and terminal job state commit or roll back together.

ABI version 23 adds read-only regeneration planning. Each plan returns the
existing artifact identity, a one-job owned set, and an owned metadata set
retargeted to that planned job so callers can inspect or explicitly enqueue the
complete request without an implicit write.

ABI version 24 replaces complete dependent and job enumerations with bounded
keyset pages, adds forward direct/transitive dependency queries, and exposes
borrowed opaque continuation cursors. A cursor remains valid only while its
owning result-set handle is live, so callers must copy it before releasing the
set. Dependency pages separately report when their depth or
visited-representation bound truncated traversal.

ABI version 25 adds bounded pages for assets, representations, resources,
locators, metadata, activity relations, provenance, stale artifacts, and changed
objects. It also adds exact activity-kind and tool-output filters, unresolved and
media-root queries, and logical-root evidence when confirming a locator. Opaque
page cursors borrow their result handles; provenance and stale-artifact pages
report traversal truncation separately from ordinary pagination.

ABI version 26 adds change delivery. `pp_production_changes_since_filtered`
returns revisions containing at least one event of the requested kinds, with a
through sequence that is the next cursor. Revision waiters block for at most
60 seconds until revisions after a sequence exist, observing commits from the
same production immediately and from other processes by polling, and report
timed-out, closed, and cancelled outcomes explicitly.

ABI version 27 adds point reads. `pp_production_asset`,
`pp_production_representation`, and `pp_production_job` return a one-element
asset, representation, or job set and report an absent identity as
`PP_ERROR_NOT_FOUND`. `pp_production_representations_using_resource` pages the
representations that use a resource, because a resource may be shared.

ABI version 28 adds the evidence kinds `PP_EVIDENCE_FINGERPRINT_MISMATCH` and
`PP_EVIDENCE_FINGERPRINT_NOT_VERIFIED`. The second marks a candidate or known
locator whose stored fingerprints all lie in domains PostProject cannot
compute; its detail lists those domains for the caller to check (ADR 0028).

ABI version 29 adds content fingerprints and observation (ADR 0029).
`pp_fingerprint_file` returns an owned `pp_fingerprint_t` holding the value
import records for a file; `pp_fingerprint_get` borrows its algorithm and bytes
until `pp_fingerprint_release`. `pp_production_verify_resource` compares
present content with stored fingerprints and reports
`PP_CONTENT_MATCHES`, `PP_CONTENT_DIFFERS`, or `PP_CONTENT_NOT_COMPARABLE`.
`pp_transaction_observe_resource_content` fingerprints present content when it
is called, outside the production lock, and stages the resource value with
every representation value recomputed from it.

ABI version 30 adds a nullable `qualifier` to
`pp_production_find_by_external_identifier`. NULL matches any qualifier; a
string matches only identifiers with exactly that qualifier (ADR 0002).

ABI version 31 adds `pp_file_path_to_locator` and `pp_locator_to_file_path`
(ADR 0030). Owned strings returned through `char **` outputs, including host
bindings, are released with `pp_string_release`, which replaces
`pp_host_binding_release`.

ABI version 32 replaces `pp_production_resolve_asset` and
`pp_media_root_mapping_t` with `pp_production_resolve_assets` and an opaque
`pp_resolution_options_t` (ADR 0031). The options carry root mappings, unnamed
search directories, the verification tier, per-directory limits, and an
optional `pp_cancel_token_t`. A cancelled token makes the call fail with
`PP_ERROR_CANCELLED`. Representation results report their asset, candidates
report the logical root they were found under, and a directory over its budget
adds `PP_EVIDENCE_SEARCH_TRUNCATED`.

ABI version 33 adds a required `pp_content_observation_t` output to
`pp_transaction_observe_resource_content` (ADR 0035). It reports
`PP_OBSERVATION_UNCHANGED` when the content matches a stored fingerprint in a
domain PostProject computes, `PP_OBSERVATION_CHANGED` when it differs, and
`PP_OBSERVATION_FIRST` when no such fingerprint was stored. An unchanged
observation records no fingerprint.

ABI version 34 replaces the single-file `pp_transaction_import_media` and the
four `pp_transaction_add_*_representation` functions with media sources
(ADR 0037). An opaque `pp_media_source_t`, created by
`pp_media_source_create_file`, `pp_media_source_create_image_sequence`,
`pp_media_source_create_ordered_parts`, or `pp_media_source_create_package`
and released with `pp_media_source_release`, describes a single file, an
image sequence, ordered parts, or a package. `pp_transaction_import_media`
takes a source and creates an asset whose original representation has its
structure; `pp_transaction_add_representation` adds a representation of a
chosen kind with the source's structure. A transaction borrows a source only
for the call.

ABI version 35 moves image-sequence file names from the sequence descriptor to
its locators (ADR 0038). A flat `pp_sequence_naming_t` holds a prefix, suffix,
and padding. `pp_representation_set_get_sequence` loses its name outputs;
`pp_representation_set_get_locator`, `pp_locator_query_set_get`, and
`pp_resolution_set_get_candidate` report an optional naming through an
`out_has_sequence_naming` flag and a borrowed `pp_sequence_naming_t`.
`pp_transaction_confirm_locator` takes a nullable root name and a nullable
naming, required exactly for a sequence resource, and replaces
`pp_transaction_confirm_locator_under_root`.
`pp_media_source_create_image_sequence` takes the naming as a
`pp_sequence_naming_t`, and `pp_production_verify_resource` and
`pp_transaction_observe_resource_content` take a nullable naming for a
sequence directory, where NULL means the naming recorded for that directory.

ABI version 36 adds bounded known-media lookup by exact current locator
identity or current effective resource fingerprint. The owned result set lends
asset, representation, and resource UUIDs plus its continuation cursor. Lookup
returns every candidate and never performs adoption or another mutation
(ADR 0041).

ABI version 37 adds transactions with an optional durable base revision and
borrowed structured semantic-conflict detail on the owned error object. A
conflict identifies its key, affected object, base revision, and superseding
revision without requiring callers to parse diagnostic text (ADR 0042).

## External identifiers

External identifiers are staged with a typed object reference, scheme, opaque
value, and optional qualifier. Add/remove operations are atomic with every other
transaction mutation. Enumeration returns an owned result-set handle whose
strings remain borrowed until release. Exact scheme/value lookup returns a
separate owned object-reference set; a NULL qualifier matches any qualifier.
Lookup does not normalize inputs or contact a registry.
