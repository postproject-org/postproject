# ADR 0020: Integration-preview compatibility tier

- Status: Accepted
- Date: 2026-09-23
- Amended: 2026-10-04

## Context

Pre-1.0 redesign remains valuable, but release 0.3 introduces integrations in
repositories with their own maintenance schedules. Requiring every experiment to
follow every PostProject commit would make external validation needlessly costly.
A blanket stability promise would be equally premature because most of the model
has not yet met a maintained consumer.

## Decision

Starting with `0.3.0-alpha.1`, PostProject defines an integration-preview subset.
That subset remains source and behavior compatible within the `0.3.x` package
series. `0.4.0` may revise it after documenting migration. This is not a 1.0 ABI
or schema promise.

The subset consists of:

- production create, open, identity, and display-name reads;
- transaction begin, commit, rollback, and revision context;
- asset and representation enumeration and structure inspection;
- media-root enumeration and machine-local root mapping inputs;
- non-mutating asset resolution and explicit locator confirmation;
- portable host-object reference formatting and parsing; and
- stable error ownership and native handle lifetime/threading rules.

The C declarations implementing these operations, their C++ and Python wrappers,
and their documented CLI workflows move together. Additive functions and result
fields are allowed within the series. Removing a named operation, changing an
existing function signature, reinterpreting a field, or narrowing accepted valid
input waits for the next minor series unless a security or data-corruption defect
requires an exception.

All other APIs remain experimental. The SQLite schema may advance within `0.3.x`
provided every released `0.3.x` production has a tested forward migration and no
production knowledge is silently discarded.

The tier ends with the `0.3.x` series. Release 0.4 names no subset: every API
of the `0.4.x` series is experimental and may change within it. Changes to
operations of the former subset are still documented with migration notes in
the changelog, so code written against `0.3.x` can be updated. A later release
may name a subset again once maintained consumers show which operations have
settled.

### Release 0.5 series

Release 0.5 names no compatibility subset. All APIs in the `0.5.x` series
remain experimental and may change within the series, with migration notes for
operations used by maintained integrations.

ADR 0040 evidence found the external-identifier and resolution operation
families mechanically eligible across the 0.4-to-0.5 boundary. They are not
named as a compatibility promise because each depends on production lifecycle,
and external-identifier mutation also depends on transaction lifecycle; those
foundational families did not have complete normal-path evidence from two
independent hosts. Publishing only the eligible leaf families would therefore
be misleadingly narrow.

Released production schemas continue to receive tested forward migrations
under the repository's pre-1.0 migration policy. This decision creates no ABI
or schema compatibility promise beyond that policy.

### Release 0.6 series

Release 0.6 names `cpp-result-propagation` as a source-level C++17 compatibility
family: `Result<T>`, `Result<void>`, `POSTPROJECT_TRY` and `POSTPROJECT_TRY_ASSIGN`
retain their success/error ownership and propagation protocol within `0.6.x`.
Diagnostic text and compiler-specific layouts are not promised. No C ABI
operation family or cross-series binary replacement promise is named.
Other APIs remain experimental with migration notes for maintained consumers.

The maintainer approved this scope on 2026-10-04. Complete Kdenlive and Natron
projection use, unchanged baseline declarations and the dependency review are
recorded in the {doc}`../release-0.6-compatibility-decision`.

The standards-impact review found no change to identifier mappings,
normalization, cardinality or domain semantics. This decision names an
existing C++ propagation protocol; it changes no API, ABI or schema version.

## Alternatives considered

- **Keep unrestricted pre-1.0 breakage.** This transfers integration churn to
  external maintainers and undermines the experiment.
- **Freeze the complete public surface.** Most APIs lack the usage evidence
  needed to justify that cost.
- **Promise ABI compatibility for all `0.x`.** This is broader and longer-lived
  than the stated minor-series learning window.

## Consequences

Release notes identify changes inside the subset, and CI runs downstream
experiments against the development branch. Integrators can pin a `0.3.x`
release without tracking every commit. APIs outside the list must be described
as experimental, not implicitly stable. A later 1.0 review may keep, revise, or
replace this tier based on actual adoption evidence.
