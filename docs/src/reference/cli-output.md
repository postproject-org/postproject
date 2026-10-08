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

## Development metadata exchange

`exchange inspect production.pproj` reports source scope, persisted role,
the retained floor and the observed revision head. The head is not yet a
complete replay feed. `exchange submit production.pproj proposal.json` accepts
the development protocol's metadata commands; its scoped base comes from the
proposal. Global `--decision-base` and `--base-revision` options reject.

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

Checkpoint exchange, complete catch-up and worker credential submission remain
unavailable in this development slice.
