# Architecture decision records

An ADR records one significant design decision: the context, the choice, the alternatives weighed, and the consequences. ADRs explain **why** PostProject behaves as it does. Current behavior is defined by the concept pages and the API reference; when an ADR and those pages disagree about the present, the present-tense pages win and the ADR is amended.

Read the relevant ADR before changing an invariant, a boundary, or a public contract. A change that alters a decision amends that record or adds a new one.


## Identity and the media model

- {doc}`ADR 0001 — Project naming </adr/0001-project-naming>`
- {doc}`ADR 0002 — Internal and external identity </adr/0002-identity-model>`
- {doc}`ADR 0003 — Compound media model </adr/0003-compound-media-model>`
- {doc}`ADR 0011 — Host-object binding </adr/0011-host-object-binding>`
- {doc}`ADR 0012 — Root container naming </adr/0012-root-container-naming>`
- {doc}`ADR 0015 — Availability semantics and collection fingerprints </adr/0015-availability-and-collection-fingerprints>`
- {doc}`ADR 0017 — Portable root identity </adr/0017-portable-root-identity>`
- {doc}`ADR 0028 — Comparable and foreign fingerprint domains in resolution </adr/0028-comparable-and-foreign-fingerprint-domains>`

## Knowledge, history, and change

- {doc}`ADR 0004 — Standards-aware metadata </adr/0004-metadata-model>`
- {doc}`ADR 0005 — Activity-based provenance </adr/0005-provenance-model>`
- {doc}`ADR 0006 — Rational time </adr/0006-rational-time>`
- {doc}`ADR 0007 — Revision and event model </adr/0007-revision-event-model>`
- {doc}`ADR 0013 — Namespace policy </adr/0013-namespace-policy>`
- {doc}`ADR 0021 — Fingerprint observations and activity snapshots </adr/0021-fingerprint-observations-and-activity-snapshots>`
- {doc}`ADR 0022 — Managed-artifact state and staleness </adr/0022-managed-artifact-state-and-staleness>`
- {doc}`ADR 0024 — Dependency relationships </adr/0024-dependency-relationships>`
- {doc}`ADR 0027 — Change delivery </adr/0027-change-delivery>`

## Media services and work

- {doc}`ADR 0018 — Discovery index location </adr/0018-discovery-index-location>`
- {doc}`ADR 0019 — Media inspection boundary </adr/0019-media-inspection-boundary>`
- {doc}`ADR 0023 — Job model and execution boundary </adr/0023-job-model-and-execution-boundary>`
- {doc}`ADR 0025 — Reference local executor </adr/0025-reference-local-executor>`

## Public surfaces, persistence, and compatibility

- {doc}`ADR 0008 — Python binding over the C ABI </adr/0008-python-binding-over-c-abi>`
- {doc}`ADR 0009 — Explicit SQL without an ORM </adr/0009-explicit-sql-no-orm>`
- {doc}`ADR 0010 — OpenAssetIO boundary </adr/0010-openassetio-boundary>`
- {doc}`ADR 0014 — Native concurrency contract </adr/0014-native-concurrency-contract>`
- {doc}`ADR 0016 — Documentation site generator </adr/0016-documentation-site-generator>`
- {doc}`ADR 0020 — Integration-preview compatibility tier </adr/0020-integration-preview-compatibility>`
- {doc}`ADR 0026 — Domain query cursors </adr/0026-domain-query-cursors>`

```{toctree}
:hidden:

/adr/0001-project-naming
/adr/0002-identity-model
/adr/0003-compound-media-model
/adr/0004-metadata-model
/adr/0005-provenance-model
/adr/0006-rational-time
/adr/0007-revision-event-model
/adr/0008-python-binding-over-c-abi
/adr/0009-explicit-sql-no-orm
/adr/0010-openassetio-boundary
/adr/0011-host-object-binding
/adr/0012-root-container-naming
/adr/0013-namespace-policy
/adr/0014-native-concurrency-contract
/adr/0015-availability-and-collection-fingerprints
/adr/0016-documentation-site-generator
/adr/0017-portable-root-identity
/adr/0018-discovery-index-location
/adr/0019-media-inspection-boundary
/adr/0020-integration-preview-compatibility
/adr/0021-fingerprint-observations-and-activity-snapshots
/adr/0022-managed-artifact-state-and-staleness
/adr/0023-job-model-and-execution-boundary
/adr/0024-dependency-relationships
/adr/0025-reference-local-executor
/adr/0026-domain-query-cursors
/adr/0027-change-delivery
/adr/0028-comparable-and-foreign-fingerprint-domains
```
