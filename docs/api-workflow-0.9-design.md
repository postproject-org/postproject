# Revised 0.9 workflow design

Inspected 2026-10-12 at SDK `f456958c`. The qualified ABI-53 candidate is
unpublished; its checks remain evidence for that source. This design selects
the remaining changes. Each slice needs its own installed verification before
the revised candidate can be qualified.

## Concepts and public surfaces

Rust domain types, C structs/owners, C++ values and Python UUID hints describe
the same knowledge. CLI task commands expose explicit effects. Keep the
distinctions below; simplify construction rather than reducing their meaning.

| Family | Treatment and integration depth |
|---|---|
| Production, identity, local storage | Keep as the entry point; distinguish local sharing, handoff and exchange |
| Internal IDs and object kinds | Keep semantic Rust/native IDs and Python UUID hints; heterogeneous references retain a runtime kind |
| Host bindings and external identifiers | Keep both directions of association; serialization belongs in first host recipes |
| Assets and representations | Keep; return the imported representation identity with the asset |
| Resources and membership | Keep compound content below the ordinary file path |
| Frames, rates, naming and roles | Keep exact content and location descriptors separate; no per-frame resource expansion |
| Locators, roots and mappings | Accept paths for file lookup; retain explicit URI/sequence inputs and local mappings |
| File facts, fingerprints and observations | Keep evidence domains and history; no implicit hashing in reads |
| Inspection and inventory | Keep optional, explicitly invoked adapters |
| Known-media lookup and adoption | Consolidate lookup inputs; preserve candidates, continuation and explicit host selection |
| Resolution and confirmation | Carry copied candidate evidence with the decision base; retain batch/compound decisions |
| Metadata assertions, keys and values | Add target-wide C++ reads; project only unambiguous Python values, retaining exact tagged alternatives |
| Vocabulary/identifier registries | Keep advisory and advanced; unknown valid terms round-trip |
| Rational time and ranges | Keep checked exact arithmetic and explicit frame context |
| Activities, tools, agents and parameters | Stage outputs and the actual activity together; retain multi-output graph composition |
| Authored dependencies and observations | Keep separate from provenance; absent, observed-empty and extraction-needed remain distinct |
| Managed artifacts and explanations | Keep representations with producer evidence; no new artifact identity |
| Availability and work state | Keep separate from artifact knowledge |
| Recorded recipe completeness | Clarify wording; retain all missing conditions without implying executable reproduction |
| Edits, views and detached bases | Consolidate native construction into additive, retained and detached edits |
| Receipts, revisions, origins and conflicts | Keep own/no-op receipts, terminal attempts and structured conflicts |
| Queries, pages, scans and cancellation | Keep point/page/view-owned scans; bounds and exhaustion remain separate |
| Revision feeds and observers | Keep advanced; filtered progress and wakeup-as-hint remain explicit |
| Jobs, leases and guarded completion | Keep optional; compose completion in the output edit, never on finalization |
| Proposals and outcomes | Keep advanced; staged proposals and deferred command builders have distinct purposes |
| Records, checkpoints and passive mirrors | Keep advanced; distinguish replication positions from query/notification cursors |
| Backends, ownership and errors | Keep domain interfaces and necessary owners; no generic CRUD facade |
| Standards and host adapters | Keep mappings outside core and host project semantics in each host |

## Selected task contracts

These signatures are implementation targets until their slices land. They
replace assembly in the existing API, rather than adding a parallel package.

| Task | Canonical direction | Removed caller work |
|---|---|---|
| Import | Return asset and original representation IDs from preparation/import | Listing representations to rediscover the imported one |
| Adopt | Python/C++ lookup accepts a path, exact locator identity or fingerprint | Path-to-URI and locator-wrapper construction; duplicate lookup method names |
| Relink | Select a representation explicitly; selection carries resource/candidate evidence and the original decision base | First-member assumptions and repeated candidate-field copying |
| Output | Stage one or more output sources and one actual activity with input roles, tool/agent and parameters | Trivial activity-edge wrappers and separate output/activity assembly |
| Revisions | Retain bounded scans on an explicitly owned coherent view | No new scan owner or hidden connection |

Native edit construction accepts optional origin/message at creation. C++ uses
`production.edit(context)`, `view.edit(context)` or
`production.edit(base, context)`. Remove `beginTransaction`, revision-only
construction and separate public context setters after migrating consumers.
Decision bases come from coherent reads, including an empty journal.

Lookup stays read-only. Zero, multiple or incomplete candidates remain explicit;
matching bytes never merge logical assets. Relinking requires complete required
member coverage and sufficient verification, with cancellation/truncation kept
visible. Confirmation is a separate staged write. A detached database decision
does not guarantee that external files have stayed unchanged.

Output staging does not invent jobs or authored dependencies. Record dependency
observations only after inspecting actual references. Multi-output activities
and guarded job completion remain possible in one edit. Staging errors and
failed commits cannot leave half-published outputs. Commit stays explicit;
uncommitted exit rolls back, and retries require a fresh decision and edit.

## Version and verification consequences

Keep package `0.9.0-alpha.1` / Python `0.9.0a1` while unpublished. Native
signature/removal changes require ABI/soname and loader-guard updates. Do not
bump schema 27, exchange protocol 1 or CLI JSON 1 for projection-only changes.
Version an actual encoding change independently. Preserve the original review
artifacts and publish new source/checksum evidence separately.

The repository inventory records a concrete migration task for every maintained
checkout. Test each slice with actual installed consumers, then rerun the
integrated SDK/platform gates and both host handoffs. Historical qualification
does not establish that the revised design is implemented or qualified.

Standards-policy review: these changes alter construction and teaching, not
identifier schemes, vocabulary mappings, cardinality or normalization. Preserve
unknown values and exact stored data; no new normative mapping is proposed.
