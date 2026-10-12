# 0078: Completed output registration does not require a job

Status: accepted for the development OpenAssetIO Manager 0.6.0 with SDK 0.9.

## Decision

Direct Manager registration records an already produced representation, its
traits and the publishing activity in one explicit edit. It does not create
and complete a synthetic job. A working reference returned by preflight still
names a real requested job: registration claims that job and stages output,
activity and guarded completion atomically while retaining its capability
until commit. Preserve authority time, fencing and terminal-job rejection.

This activity describes publishing by the configured host, not an inferred
render recipe. No activity input implies an authored dependency. Failure after
output staging rolls back the whole edit, including metadata and provenance.

## Impact

Direct registrations stop emitting job-request/claim/completion events; hosts
requiring durable intent use preflight. Representation references, stored media,
existing job knowledge and OpenAssetIO publishing signatures are unchanged.
No SDK ABI, schema or protocol change is needed. Manager 0.6 remains prepared
and unpublished; previously published artifacts are untouched.

Standards-policy review: no OpenAssetIO trait mapping, scheme normalization or
domain cardinality changes. This separates optional work coordination from
recording completed work through existing operations.
