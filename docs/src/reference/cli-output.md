# CLI output

Human output is intended for people. Scripts should use `--json`, optionally
with `--output-version 1`. Unsupported versions reject before opening files.
JSON format 1 keeps each command's documented shape: objects include
`format_version: 1`; arrays and scalar results retain their shape. IDs are UUID
strings, revision sequences are integers, and timestamp fields name their units.

Operation failures emit `{"format_version": 1, "error": {...}}` on stdout;
diagnostics go to stderr. `error.code` identifies the category and
`error.transaction_conflict` carries semantic conflict details when available.
Argument-parser errors emit diagnostics/help on stderr without a JSON object.
Representation, dependency and metadata JSON input files are limited to 64 MiB;
arrays also enforce their domain item limits. Invalid or oversized JSON uses
`invalid_argument` and leaves the production unchanged.

| Exit | Meaning / error codes |
|---|---|
| 0 | Success |
| 1 | Other adapter failure: `operation_failed` |
| 2 | Invalid input: `invalid_argument`, or argument parsing |
| 3 | Missing object: `not_found` |
| 4 | State conflict: `conflict`, `already_exists` |
| 5 | Ambiguous media: `ambiguous_resolution` |
| 6 | Cancelled before completion: `cancelled` |
| 7 | I/O or data failure: `io`, `storage`, `migration`, `fingerprint` |
| 8 | Unsupported operation: `unsupported` |
| 9 | Internal failure: `internal` |

Successful writes report their own `commit_receipt`; an unchanged write has no
revision. A failure after a known commit uses `post_commit_failure` with
`cause_code`, `commit_receipt` and ordered `commit_receipts`. Its exit category
describes the cause. Inspect those receipts before retrying: the earlier writes
are durable even when output delivery or later worker publication failed.

## Development portable exchange

`exchange inspect production.pproj` reports source scope, persisted role,
the retained floor, supported features and coherent observed/replay heads.
`head` is an exact scoped continuation; `replay_status: history_gap` and a null
head identify incomplete development history. `exchange position production.pproj
cursor.json` saves a complete continuation to a new file.
`exchange submit production.pproj proposal.json` accepts the development
protocol's domain commands; its scoped base comes from the proposal.
Global `--decision-base` and `--base-revision` options reject.

With `--json`, the result's `outcome` is a protocol-major-1 object inside the
CLI format-1 envelope. Protocol integers are exact decimal **strings**, including
receipt sequences. Accepted no-op outcomes have `receipt.revision: null`.
Terminal rejection returns a nonzero domain exit code and includes the retained
outcome alongside `error`. Recover it with:

```sh
postproject --json exchange outcome production.pproj \
  --history HISTORY_UUID --client CLIENT_UUID --request REQUEST_UUID
```

Lookup returns `outcome: null` for an unknown identity. It does not reapply a
request. Equivalent retries return the original result; changed intent requires
a new request UUID. `request_identity_mismatch` exits 4 and retains the original
outcome. `scope_mismatch`, `invalid_base`, `malformed` and `limit_exceeded` exit 2;
`mirror_read_only` exits 8. Proposal framing rejects duplicate keys and is
bounded to 64 MiB before opening the production.

`exchange checkpoint export production.pproj knowledge.ppxc` writes a pinned,
sealed checkpoint to a new file. `checkpoint validate knowledge.ppxc` performs
the full domain import in temporary private storage and discards it.
`checkpoint import knowledge.ppxc mirror.pproj` creates a new passive mirror;
existing destinations reject. Validation/import accept `--max-encoded-bytes`,
`--max-disk-bytes` and `--max-frames` (defaults 1 GiB, 2 GiB and 10 million).

`exchange changes export production.pproj changes.ppxd --from cursor.json`
exports complete records through one pinned head. `changes apply mirror.pproj
changes.ppxd` verifies duplicates and commits each complete record atomically.
Its `--max-encoded-bytes` defaults to 1 GiB. Earlier complete records survive a
later failure; inspect the mirror and retry the same file. These successes emit
`head` with exact protocol strings. A truncated file uses `integrity` (exit 2);
`history_gap` and `divergence` exit 4. Failed exports publish no new output.

Submission accepts repeated `--lease-token-file PATH` arguments; `-` reads one
token from stdin. New claims require `--claim-output JOB_UUID=PATH` for each
claimed job. Files are exclusive and private (0600 on Unix); public outcomes
never contain tokens. Equivalent retries return the original outcome without
reserving or reissuing claim files, including after release or expiry. Lost
delivery needs expiry or coordinator cancellation. A delivery error after commit
uses `post_commit_failure` and the original receipt; recover the public outcome
using the request identity. Mirror observations cannot authorize these actions.
