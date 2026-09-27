# ABI policy

ABI version 32 is pre-release and may change during the 0.x series, with every
change recorded in the changelog and ABI tests. `pp_abi_version()` reports the
implemented version. Exported symbol names are unversioned until the first stable
release, but removals or signature changes require an explicit ABI-version bump.

## Types and ownership

Productions, transactions, asset sets, media-root sets, representation sets,
resolution sets, activity sets, external-identifier sets, object-reference sets,
object-query sets, locator-query sets, dependency-query sets, job sets, metadata
inputs, revision waiters, and errors are opaque handles. A
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

## C++ wrapper

`postproject.hpp` is a header-only C++17 wrapper over the authoritative C API.
It owns production and transaction handles with RAII, makes both wrappers move-only,
and converts failed status codes to `postproject::Error`. Destruction of an open
transaction invokes the C release behavior and therefore discards staged work.
The exception retains the stable `ErrorCode` and copies diagnostic text before
releasing the C error object. No exception crosses the C ABI. Inputs containing
embedded NUL bytes are rejected before calling C.

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
with a pattern and frame domain rather than one synthetic resource per frame.

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

## External identifiers

External identifiers are staged with a typed object reference, scheme, opaque
value, and optional qualifier. Add/remove operations are atomic with every other
transaction mutation. Enumeration returns an owned result-set handle whose
strings remain borrowed until release. Exact scheme/value lookup returns a
separate owned object-reference set and does not normalize inputs or contact a
registry.

The wrapper adds no domain behavior and exposes no C++ standard-library type
through exported library symbols. Its source compatibility follows the 0.x
pre-release policy independently of the C ABI version.
