# 0082: Carry verified relink choices with their coherent decision

Status: accepted for development C ABI 57.

Relink callers repeat confidence checks and copy resource IDs, candidate URIs,
roots and sequence naming. Counting candidates alone loses search completeness
and the distinction between presence, sampled evidence and verified content.

Add an explicit verified selection on a representation resolution. It returns
copied locations and the original coherent decision base. Selection requires
complete required content, one fully verified candidate for every required
resource and no truncated or failed search. Unavailable or unverified optional
members remain in diagnostics and are omitted from the selection. Multiple
representations always require the caller to name the representation.

Selection reads recorded results; it performs no additional scan or media I/O.
Confirmation validates the edit's original decision base and prepares every
locator before staging any. An already recorded resource/URI/naming identity
is a guarded no-op and keeps its annotations. Commit stays explicit. Copied selections outlive
the read view and native result owner. Database guards do not make filesystem
changes atomic or replace host association/generation checks.

Keep explicit locator confirmation for independently reviewed or manually
supplied locations; it has a distinct policy and does not assert verification.
Host-specific sampled policies remain explicit: Kdenlive additionally checks
its own clip hash; Blender may reuse an already recorded sampled file address;
sequence pilots require complete frames before accepting sampled collection
evidence. None of these cases is a fully verified SDK selection.
Batch resolution and all diagnostics remain available. Native resolution
captures its decision and storage inputs from one coherent view, then releases
the view before scanning or hashing.

ABI 57 adds borrowed selection/location structs and selection/confirmation
operations; loader guards and installed consumers change together. Schema 27,
exchange framing major 1 and CLI JSON 1 are unchanged. Deferred proposals use
versioned command kind `locator.confirm.v1`; older decoders reject this kind
as unsupported. Existing command/record encodings are unchanged, and replay
captures only newly added locators. No-op confirmation retains its locator-set
guard through submission. Standards-policy review:
no identifier, vocabulary, normalization or standards mapping changes.
