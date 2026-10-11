# 0074: One commit operation returns its receipt

Status: accepted for development C ABI 53.

## Decision

Keep one `commit` operation on each surface. Rust removes the redundant
`commit_with_receipt` method. C removes the status-only commit and renames
the receipt-returning function to `pp_transaction_commit`. Its receipt
output is required; a missing output rejects before attempting a commit.
C++ and Python continue returning `CommitReceipt` from `commit`.

The atomic write path supplies the production and the revision made by that
edit. A no-change receipt has no revision. Every actual commit attempt is
terminal, including semantic conflict or storage failure; retry requires a
new decision and edit. No operation reads a later head to recover a receipt.

## Migration and standards impact

Use Rust `commit()` and C `pp_transaction_commit(edit, &receipt, &error)`.
Update all maintained callers; do not retain aliases. The development ABI
has 344 exports and 38 public layouts. Schema 27, protocol major 1, revision
meaning and effect capture are unchanged.

Reviewed against the contributor standards policy: this removes duplicate
call shapes and introduces no vocabulary, identifier or standards mapping.
