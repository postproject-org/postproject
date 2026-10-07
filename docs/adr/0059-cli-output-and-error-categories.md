# 0059: CLI output and error categories

Status: accepted for the 0.7 development SDK.

## Decision

JSON format 1 preserves command-specific result shapes. Objects include
`format_version: 1`; arrays and scalar results retain their shapes. Scripts
can require the format with `--json --output-version 1`. Unknown versions
reject at argument parsing, before opening a production.

Every operation failure emits a JSON error object when JSON is requested,
with its stable category and optional semantic conflict payload. Diagnostics
also go to stderr. Parser failures retain Clap's help/diagnostic behavior and
exit 2. Exit categories distinguish input, missing objects, conflict, ambiguity,
cancellation, storage/I/O, unsupported operations and internal failures.
Unclassified adapter failures retain exit 1 and `operation_failed`.

Failures following known commits expose `post_commit_failure`, the underlying
`cause_code`, and the exact ordered receipts. Their exit category describes
the cause; scripts must inspect receipts before retrying. No later head read
is substituted for a receipt. Claim tokens remain absent from error payloads.

## Migration and standards impact

Scripts that assumed every failure exited 1, or only semantic conflicts emitted
JSON, must use the documented categories. Object consumers should tolerate the
added format field. The published 0.6 CLI remains unchanged.

This is an internal CLI protocol decision. It introduces no external standards
mapping, identifier normalization or synchronization protocol.
