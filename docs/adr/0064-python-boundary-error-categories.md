# 0064: Python boundary error categories

Status: accepted for the 0.7 development SDK.

## Decision

Python boundary failures use the public exception hierarchy and C error codes.
Required unknown native tags or an incompatible ABI raise `UnsupportedError`.
Known tags with inconsistent payloads, missing required outputs or invalid
native text raise `InternalError`. Operations on closed owners or terminal
edits raise `InvalidArgumentError`. Check ABI compatibility before configuring
its operation signatures.

Ordinary Python type mistakes still raise `TypeError`; library discovery keeps
its existing filesystem/configuration exceptions. Cleanup remains idempotent.
The native boundary remains authoritative for operation errors. Optional
unknown vocabulary and authored external facts continue to round-trip.

## Standards impact

No schema, standards mapping or serialized identity changes. The categories
describe binding and ABI failures, not interpretation of external media facts.
