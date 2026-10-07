# 0058: Job input decisions

Status: accepted for the 0.7 development SDK.

## Decision

Claiming from a read-derived edit checks whether its input fingerprints or
complete dependency sets changed after that edit's base. Completion checks
those same facts against the claim's existing journal revision, or the edit's
older decision base. The writer lock protects the check through publication.
A claim and completion in one edit use that edit's facts atomically.

Changes to an input resource fingerprint, representation fingerprint or complete
dependency set reject publication with the existing structured semantic conflict.
Output, activity, snapshots and terminal state remain unpublished. Renewing a
lease does not refresh the worker's input decision. Reopening or token import
retains the boundary through the recorded claim event; no new token field or
schema migration is needed.

The check reads at most one changed conflict record, using bounded private-key
decoding. It does not materialize all input resources or fingerprint domains.
The claim journal is retained under the existing local history policy.

Availability, labels and host configuration remain separate from content and
dependency knowledge. This guards recorded input facts; it does not prove what
bytes an external tool actually read or prevent filesystem changes outside the
production. Workers record the parameters they actually used on their activity.

## Migration and standards impact

Existing lease signatures remain unchanged. An input-change conflict requires
fresh work and a new claim, rather than retrying publication with a newer base.
Release or cancel the old job explicitly as appropriate for the host.

Reviewed against the standards policy: this refines internal work/provenance
consistency. External vocabulary values remain opaque and unchanged; no new
standards mapping, authenticity claim or distributed read-set protocol is added.
