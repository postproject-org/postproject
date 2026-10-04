# 0046: Python identity hints and explicit object references

Status: accepted for the development API; downstream migration is in progress.

## Decision

UUID identities use `typing.NewType` over `uuid.UUID`. Returned IDs remain
ordinary UUIDs with standard equality, hashing, ordering, formatting, and
serialization. `parse_id(text, AssetId)` checks UUID syntax and supplies the
nominal hint. There is no runtime ID subclass or per-operation `UUID | str`
overload. The native boundary rejects non-UUID values before making a call.
The store remains responsible for existence and production membership.

Runtime UUIDs cannot reveal their semantic kind. Polymorphic targets therefore
use explicit frozen alternatives: `ProductionRef`, `AssetRef`,
`RepresentationRef`, `ResourceRef`, `ActivityRef`, and `JobRef`. Each contains
its corresponding nominal ID. Metadata targets, dependencies, host bindings,
events, and object queries preserve that alternative when copied from C.
Reference constructors reject non-UUID payloads. Deliberately retagging a UUID
cannot establish that it names a stored object of the requested kind.

## Migration and standards impact

Replace `asset_id.value` with `asset_id`. Replace dynamic targets such as
`add_metadata(asset_id, ...)` with `add_metadata(AssetRef(asset_id), ...)`.
Use `isinstance(id, UUID)` for runtime value checks and variant checks for
object references. Installed hints distinguish ID kinds through `py.typed`;
type checking remains optional for applications.

These are Python source changes. Stored UUID bytes, host-binding text, external
identifiers, and schema stay unchanged. Reviewed against the contributor
standards policy: no normative mapping or external vocabulary changes.
