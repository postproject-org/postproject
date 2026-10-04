# Native host actions and cleanup

Two optional host pilots show installed C and C++ integration with distinct
ownership and scheduling needs. Both use explicit local productions. Their
complete programs, including the failure branches, are executable downstream
tests against an install prefix; no consumer invokes Cargo.

## C: register a finalized recording

The [OBS registration adapter](https://github.com/postproject-org/postproject-obs/blob/main/src/registration.c)
owns a production, media source, transaction, result sets, metadata inputs and
errors. Each has one matching public release function. Its single cleanup path
releases partially acquired handles on failure. It copies the error message
before releasing the error. Strings returned by a result set remain borrowed
until that set is released; only copied IDs and a diagnostic leave the adapter.

The ordinary path imports the successfully finalized file and commits its
qualified attempt identifier and technical metadata together. File hashing
occurs during staging on an adapter worker. The representation is available
after that commit; a second transaction attaches the capture activity. If the
asset has acquired a proxy, readback still selects its unique original rather
than assuming result order identifies it. Ambiguity requires review. If the
second stage fails, explicit retry uses the same attempt identifier to complete
the fact without importing another asset. This is a bounded retry policy,
not a promise about arbitrary crashes or concurrent first registration.

The [C contract test](https://github.com/postproject-org/postproject-obs/blob/main/tests/registration.c)
executes failed staging followed by rollback, successful registration, repeated
notification and lost acknowledgement. It compiles the same adapter used by
OBS. The host acceptance additionally checks playable recordings after
registration failure, explicit menu retry and shutdown during work.

Before commit, a failed staging operation leaves an open transaction that can
be rolled back. Commit and rollback are terminal, including failed commit:
release their handles without rolling them back again. A closed handle may
remain alive while another transaction is open. Releasing it must not clear
that transaction's guard; the installed C/C++ lifetime regressions verify
commit, rollback and failed-commit variants.

## C++: retain a sequence decision

The [Natron C++ adapter](https://github.com/postproject-org/postproject-natron/blob/main/src/resolution.cpp)
uses owning wrapper values and `Result<T>` with both propagation helpers.
Resolution follows this recipe:

1. Copy the host object's identity, association and filename on the host thread.
2. Read a decision base before the native sequence and locator reads.
3. Resolve presence; verify content only when the selected sequence is complete.
4. Read the revision again. Discard the decision if the production changed.
5. On the host thread, discard the result after cancellation, node deletion or
   a changed association/filename.
6. Confirm a new, unambiguous locator in a transaction using the retained base.
   An already recorded location needs no mutation.
7. Copy the candidate pattern into the Reader only after successful confirmation.

The [installed C++ recipe](https://github.com/postproject-org/postproject-natron/blob/main/tests/contract.cpp)
tests sequence adoption, owning values, an empty production without a base,
rollback, a directory move, explicit verification, partial refusal and a
separate writer invalidating a retained decision. A conflict closes the
transaction; destruction releases it. Re-read and decide with fresh values.
Changing only the old decision's base would bypass its intended protection.

The real Natron tests use genuine padded PNG frames and a native Reader/Writer.
They preserve the canonical binding and ordinary filename through save/reopen.
The scripting bridge uses standard strings, dictionaries, fractions and a
joined executor; every production operation stays in the installed C++ adapter.

## Revision and result boundaries

Neither pilot identifies its own commit by fetching the latest revision later.
Natron's retained cursor describes fully processed revisions. For filtered
feeds, advance the returned `through_sequence` even on an empty page; it is
the continuation watermark. A waiter can wake a host, but the durable feed
cursor is still needed for restart and catch-up. See {doc}`revision-feed` and
its tested examples for cancellation and close.

A resolver does not mutate a production. Partial availability and ambiguity
require a host decision and preserve the native fallback. Fingerprint I/O
already running may delay a joined shutdown; detaching work into an unloaded
plugin is not a valid timeout policy. Origin identifies the application and
version, rather than an authenticated person.
