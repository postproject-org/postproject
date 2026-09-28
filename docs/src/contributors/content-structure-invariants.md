# Content-structure invariants

The durable ownership chain is `Asset -> Representation -> ContentStructure ->
Resource -> Locator`. Keep these levels distinct in domain, persistence, and
public APIs.

An asset is logical production identity. A representation is one usable
realization of that asset. A resource identifies stored content participating
in the realization, while a locator is only an access route. Moving content may
replace a locator; it must not replace the resource, representation, or asset.

Every representation has exactly one explicit structure:

- `SingleResource` references exactly one resource.
- `ImageSequence` references one compact patterned resource.
- `OrderedParts` contains required members whose order is semantic.
- `Package` contains role-bearing required or optional members whose order is
  not semantic.

All referenced resources must exist, belong to the representation exactly once,
and have at least one locator when a representation is created. Ordered parts
must remain in their declared order. A package must contain at least one
required member. Roles are namespaced open-world identifiers, not vendor enums.

A regular image sequence stores an inclusive stepped frame domain, rational
rate, and bounded sorted exceptions. It does not persist one resource row per
frame. Every locator of the sequence, and no other locator, carries a sequence
naming: prefix, suffix, and padding (ADR 0038). Presence is determined with one
directory inventory: expected filenames are generated from the locator's naming
and the descriptor, recorded exceptions are excluded, and observed gaps are
returned as sorted frame diagnostics.

Resource fingerprints cover storage-level content evidence. Representation
fingerprints combine canonical structure with resource evidence and exclude
object IDs and locators so identity survives relocation. Ordered-part
fingerprints preserve member order; package fingerprints do not. Sequence
fingerprints sample deterministic members and record their strategy rather than
claiming complete-content verification. Neither contains file names.

Availability aggregates required resource outcomes with the precedence `Error
> Ambiguous > Offline > Partial > Online`. Optional package gaps remain issues
but do not reduce availability. Resolver code must never hide ambiguity or turn
one available member into a complete representation.

Changes to these rules require an ADR update, domain tests, persistence/reopen
coverage, and coordinated C, C++, Python, CLI, and documentation changes where
the public surface is affected.
