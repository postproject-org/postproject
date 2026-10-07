# 0057: Authority-controlled job leases

Status: accepted for development; cross-surface migration is in progress.

## Decision

A claim returns an owning, production-bound lease. Claim secrets remain private;
job observations contain attribution and expiry. The existing random claim UUID
supplies fencing without another generation counter. Each claimant transition
checks production, job, current claim, state and expiry under the writer lock.
Coordinator cancellation continues to use the job ID.

Claim and renewal accept positive whole-microsecond durations up to 24 hours.
No rounding or caller-selected current time is supported.
Rust accepts `Duration`; C++ accepts integral `std::chrono::duration` counts,
Python accepts `timedelta`, and C names its scalar microsecond unit explicitly.
Floating/custom C++ count types and fractional-microsecond durations reject.
The SQLite authority samples its private clock on transitions and again before
commit. Renewal must
extend the current expiry and commit before that previous expiry. Release also
requires an unexpired claim.

The authority records a durable wall-clock high-water mark. A backward jump
rejects guarded operations until the clock catches up; forward jumps can expire
claims immediately. Failed guarded commits roll back publication before
preserving the observed high-water mark. Reopening retains that mark. This is a
conservative local-clock policy, not an elapsed-time or trusted-time guarantee.
An interrupted, unacknowledged operation supplies no durable time observation.

A new lease is pending until its edit commits. That same edit can claim and
complete atomically, preserving Manager metadata/provenance publication.
Failed claiming edits close pending leases; failed operations on an existing
lease leave local ownership unchanged and subsequent use checks the store again.
Successful release, failure or completion closes ownership. Freeing/dropping a
lease only frees local memory; it never writes or renews.

Reference-executor requests own a noncredential execution nonce and are consumed
once. Both temporary and final filenames include it, isolating different workers'
outputs for the same job. Rejected completion cleans up only its own attempt.
Filesystem execution and database completion remain separate; a crash can leave
unregistered files, without publishing partial production facts.

Explicit token export/import supports workers spanning processes. Its bounded,
versioned encoding includes production, job and secret; import validates all
three against current authoritative state. CLI accepts token files/stdin,
never ordinary argument values. Listings, events and diagnostics omit secrets.
Claim reserves an exclusive private output file before commit; credential delivery
happens only after activation. Delivery failure retains the exact committed
receipt and leaves the claim durable, with expiry or explicit coordinator
cancellation as recovery. It removes the incomplete owned file and does not
silently compensate for an already committed operation.

## Migration and standards impact

Replace claim-ID/time pairs with leases and native duration values. Existing
claims expire during the schema migration; requested and terminal jobs remain.
Changed C declarations require a coordinated ABI bump and matching projections.

Reviewed against the standards policy: lease authority is internal workflow
coordination. It adds no standards mappings, authentication or provenance
authenticity claim.
