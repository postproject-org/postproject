# ADR 0067: Native portable exchange projection

Status: accepted for development; projections and qualification in progress.

## Decision

C exposes copied, distinct history/client/request IDs, source-scoped decisions
and replay positions. Inspection pins one view and reports the native observed
decision separately from an available complete replay head. An absent complete
head identifies a history gap; it never invents a digest or performs recovery.
C++ copies these values and uses Result/RAII. Python uses UUID NewType hints and
ordinary exact integer values. These experimental additions name no stability
promise and preserve the existing Result propagation contract.

Prepare native proposals from the existing checked transaction staging calls.
Conversion retains command order, prepared IDs and revision context, creates no
domain revision, and closes the staging transaction on success. Pending native
claims close during conversion; only a successful fresh protocol submission
delivers new owning leases. A failed conversion leaves the staging edit open.
Ordinary transaction commit remains available with its current semantics.

A supplied protocol decision keeps its production/history scope and must match
the staging edit's decision. Binding an ordinary local DecisionBase verifies
membership in the opened source history. A mirror-derived protocol decision
must never be rebound by discarding its history. Immutable proposals can also
be transported through bounded canonical bytes, but normal native commands use
the typed staging inputs rather than JSON.

Python additionally accepts ordered collections of typed local command values.
A builder reuses native staging validators and returns typed prepared IDs for
later commands. Paths and owning leases are local preparation inputs only;
portable intent contains measured facts and public job identity. Closing a
builder discards staging. After a staging failure, discard it before making a
new decision. Submission still needs explicit private context.

Public outcomes and fresh private lease delivery remain separate. Duplicate
submission checks the original private ownership binding before current-time
authorization and returns no new ownership. Public lookup never returns a
credential. All protocol failures have numeric error categories; storage errors
retain existing codes and structured transaction conflict details.

File exchange reuses the checked Rust receiver and pinned exporter. Outputs
publish exclusively after a complete seal and flush. Imports create a new
passive mirror; catch-up preserves atomic records and their durable prefix.
Budgets are positive checked values. Foreign scope, gaps, corrupt framing and
ordinary mirror writes remain explicit failures.

## Architecture and standards

The FFI adapter may depend directly on the protocol codec for checked projection;
the architecture gate records this edge. Core and media dependency directions
are unchanged. No new external standards mapping or normalization is introduced.
Unknown portable extension/vocabulary facts retain ADR 0065's exact spelling;
the C values do not imply authentication, network transport or worker authority.
