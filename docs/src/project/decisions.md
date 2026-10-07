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
- {doc}`ADR 0030 — Canonical locator spelling for hosts </adr/0030-canonical-locator-spelling>`
- {doc}`ADR 0037 — Media sources for import and representations </adr/0037-media-sources>`
- {doc}`ADR 0038 — Image-sequence file names belong to the locator </adr/0038-sequence-names-on-locators>`

## Knowledge, history, and change

- {doc}`ADR 0004 — Standards-aware metadata </adr/0004-metadata-model>`
- {doc}`ADR 0005 — Activity-based provenance </adr/0005-provenance-model>`
- {doc}`ADR 0006 — Rational time </adr/0006-rational-time>`
- {doc}`ADR 0007 — Revision and event model </adr/0007-revision-event-model>`
- {doc}`ADR 0013 — Namespace policy </adr/0013-namespace-policy>`
- {doc}`ADR 0021 — Fingerprint observations and activity snapshots </adr/0021-fingerprint-observations-and-activity-snapshots>`
- {doc}`ADR 0029 — Content fingerprints and observation for hosts </adr/0029-host-content-observation>`
- {doc}`ADR 0035 — Reporting the outcome of a content observation </adr/0035-content-observation-outcome>`
- {doc}`ADR 0022 — Managed-artifact state and staleness </adr/0022-managed-artifact-state-and-staleness>`
- {doc}`ADR 0024 — Dependency relationships </adr/0024-dependency-relationships>`
- {doc}`ADR 0027 — Change delivery </adr/0027-change-delivery>`

## Media services and work

- {doc}`ADR 0018 — Discovery index location </adr/0018-discovery-index-location>`
- {doc}`ADR 0019 — Media inspection boundary </adr/0019-media-inspection-boundary>`
- {doc}`ADR 0031 — Resolution scope, budgets, batches, and cancellation </adr/0031-resolution-scope-and-batches>`
- {doc}`ADR 0023 — Job model and execution boundary </adr/0023-job-model-and-execution-boundary>`
- {doc}`ADR 0025 — Reference local executor </adr/0025-reference-local-executor>`

## Public surfaces, persistence, and compatibility

- {doc}`ADR 0008 — Python binding over the C ABI </adr/0008-python-binding-over-c-abi>`
- {doc}`ADR 0009 — Explicit SQL without an ORM </adr/0009-explicit-sql-no-orm>`
- {doc}`ADR 0010 — OpenAssetIO boundary </adr/0010-openassetio-boundary>`
- {doc}`ADR 0014 — Native concurrency contract </adr/0014-native-concurrency-contract>`
- {doc}`ADR 0032 — C++ error reporting without exceptions </adr/0032-cpp-error-reporting>`
- {doc}`ADR 0033 — Propagating C++ results </adr/0033-cpp-result-propagation>`
- {doc}`ADR 0034 — A warning-free C++ header in host builds </adr/0034-warning-free-cpp-header>`
- {doc}`ADR 0036 — Readable C++ metadata values </adr/0036-readable-cpp-metadata-values>`
- {doc}`ADR 0039 — Platform wheels carrying the native library </adr/0039-platform-wheels>`
- {doc}`ADR 0016 — Documentation site generator </adr/0016-documentation-site-generator>`
- {doc}`ADR 0020 — Integration-preview compatibility tier </adr/0020-integration-preview-compatibility>`
- {doc}`ADR 0026 — Domain query cursors </adr/0026-domain-query-cursors>`
- {doc}`ADR 0040 — ABI usage evidence and compatibility families </adr/0040-usage-evidence-and-compatibility-families>`
- {doc}`ADR 0041 — Known-media adoption </adr/0041-known-media-adoption>`
- {doc}`ADR 0042 — Semantic optimistic write conflicts </adr/0042-semantic-write-conflicts>`
- {doc}`ADR 0043 — Local shared productions </adr/0043-local-shared-productions>`
- {doc}`ADR 0044 — Closed transaction guard ownership </adr/0044-closed-transaction-guard-ownership>`
- {doc}`ADR 0045 — Coherent reads and commit receipts </adr/0045-read-views-and-commit-receipts>`
- {doc}`ADR 0046 — Python UUID hints and explicit references </adr/0046-python-identities-and-object-references>`
- {doc}`ADR 0047 — Local cursor scope and lifetime </adr/0047-local-query-cursor-scopes>`
- {doc}`ADR 0048 — Immediate option validation </adr/0048-immediate-native-options-validation>`
- {doc}`ADR 0049 — Native semantic identities </adr/0049-native-semantic-identities>`
- {doc}`ADR 0050 — Bounded revision events </adr/0050-bounded-revision-events>`
- {doc}`ADR 0051 — Closed job-state projections </adr/0051-closed-job-state-projections>`
- {doc}`ADR 0052 — Mergeable metadata and destructive decisions </adr/0052-metadata-append-conflicts>`
- {doc}`ADR 0053 — Fresh bounded media-root reads </adr/0053-fresh-bounded-media-root-reads>`
- {doc}`ADR 0054 — C++ dynamic reference values </adr/0054-cpp-dynamic-reference-values>`
- {doc}`ADR 0055 — CLI dependency submission outcomes </adr/0055-cli-dependency-submission-outcomes>`
- {doc}`ADR 0056 — Decision-bound content observations </adr/0056-decision-bound-content-observations>`
- {doc}`ADR 0057 — Authority-controlled job leases </adr/0057-authority-controlled-job-leases>`
- {doc}`ADR 0058 — Job input decisions </adr/0058-job-input-decision-guards>`
- {doc}`ADR 0059 — CLI output and errors </adr/0059-cli-output-and-error-categories>`

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
/adr/0029-host-content-observation
/adr/0030-canonical-locator-spelling
/adr/0031-resolution-scope-and-batches
/adr/0032-cpp-error-reporting
/adr/0033-cpp-result-propagation
/adr/0034-warning-free-cpp-header
/adr/0035-content-observation-outcome
/adr/0036-readable-cpp-metadata-values
/adr/0037-media-sources
/adr/0038-sequence-names-on-locators
/adr/0039-platform-wheels
/adr/0040-usage-evidence-and-compatibility-families
/adr/0041-known-media-adoption
/adr/0042-semantic-write-conflicts
/adr/0043-local-shared-productions
/adr/0044-closed-transaction-guard-ownership
/adr/0045-read-views-and-commit-receipts
/adr/0046-python-identities-and-object-references
/adr/0047-local-query-cursor-scopes
/adr/0048-immediate-native-options-validation
/adr/0049-native-semantic-identities
/adr/0050-bounded-revision-events
/adr/0051-closed-job-state-projections
/adr/0052-metadata-append-conflicts
/adr/0053-fresh-bounded-media-root-reads
/adr/0054-cpp-dynamic-reference-values
/adr/0055-cli-dependency-submission-outcomes
/adr/0056-decision-bound-content-observations
/adr/0057-authority-controlled-job-leases
/adr/0058-job-input-decision-guards
/adr/0059-cli-output-and-error-categories
```
