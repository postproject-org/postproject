# Project and engineering documentation

This section is for contributors, maintainers, reviewers, and integrators who need to understand why PostProject is built the way it is.

It is intentionally **not** the starting point for ordinary users. Architecture notes, persistence details, benchmarks, release reports, and ADRs are important records, but they should not sit between a new reader and the basic PostProject model.

## Current direction

Read {doc}`../../roadmap` for planned work and release goals. The roadmap explains where the project is going; it should not be used as a substitute for the concept documentation that defines current behavior.

## Contributing

Start with {doc}`../contributors/README`.

Changes to domain meaning, persistence, public APIs, or the ABI often need coordinated work across several layers. The contributor pages describe:

- content-structure invariants;
- metadata model rules;
- provenance model rules;
- standards policy.

## Architecture and implementation reference

Use these pages when the implementation itself is relevant:

- {doc}`../../architecture`
- {doc}`../../domain-model`
- {doc}`../../media-resolution`
- {doc}`../../persistence`
- {doc}`../../testing`
- {doc}`../../benchmarks`
- {doc}`../../compatibility-evidence`
- {doc}`../../fuzzing`

These are engineering references. Application integrators should prefer the integrator and concept guides unless they are diagnosing implementation behavior.

## Compatibility and releases

{doc}`releases` collects the native compatibility contract, the release process, and the acceptance report of every release.

Historical reports should remain available because they are useful evidence, but they should be treated as history rather than mixed into the primary learning path.

## Architecture Decision Records

{doc}`decisions` lists every ADR by area. ADRs explain significant design choices at the time they were made. They are valuable when changing an invariant or revisiting a boundary, but they are not normative onboarding documentation.

Use the current concept/API documentation for present behavior, then consult the relevant ADR to understand the reasoning and tradeoffs behind it.

## Standards impact

When a change touches an area that maps to an external standard, consult the standards policy and standards-impact records before extending the core model. The default preference is to keep external-standard-specific behavior at adapter boundaries unless the concept is genuinely part of PostProject's application-neutral production model.

```{toctree}
:hidden:

../contributors/README
/architecture
/domain-model
/media-resolution
/persistence
/testing
/benchmarks
/compatibility-evidence
/fuzzing
releases
decisions
```
