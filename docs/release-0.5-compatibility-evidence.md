# Compatibility-family usage evidence

Baseline: `0.4.0-alpha.1`. Candidate: `0.5.0-alpha.1`.

| Family | Last change | ABI | blender | kdenlive | openassetio-manager | Mechanically eligible |
|---|---|---:|---|---|---|---|
| `production-lifecycle` | `0.4.0-alpha.1` | 32 | — | — | ABI | no |
| `transaction-lifecycle` | `0.4.0-alpha.1` | 32 | — | ABI | — | no |
| `asset-point-read` | `0.4.0-alpha.1` | 26 | — | — | ABI | no |
| `external-identifiers` | `0.4.0-alpha.1` | 30 | ABI | ABI | — | yes |
| `locator-inspection` | `0.4.0-alpha.1` | 35 | — | — | — | no |
| `resolution` | `0.4.0-alpha.1` | 35 | ABI | ABI | ABI | yes |
| `revision-feed` | `0.4.0-alpha.1` | 28 | — | ABI | — | no |
| `content-verification` | `0.4.0-alpha.1` | 29 | ABI | — | — | no |
| `host-object-binding` | `0.4.0-alpha.1` | 32 | — | — | ABI | no |
| `cpp-result-propagation` | `0.4.0-alpha.1` | 32 | — | projection | — | no |

## production-lifecycle

C ABI: `pp_production_create`, `pp_production_open`, `pp_production_id`, `pp_production_release`

C++ projection: `Production::create`, `Production::open`, `Production::id`

Python projection: `Production.create`, `Production.open`, `Production.id`

Hosts with complete mechanical use evidence: `openassetio-manager`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=1.

## transaction-lifecycle

C ABI: `pp_production_begin_transaction`, `pp_transaction_set_revision_context`, `pp_transaction_commit`, `pp_transaction_rollback`, `pp_transaction_release`

C++ projection: `Production::beginTransaction`, `Transaction::setRevisionContext`, `Transaction::commit`, `Transaction::rollback`

Python projection: `Production.transaction`, `Transaction.set_revision_context`, `Transaction.commit`, `Transaction.rollback`

Hosts with complete mechanical use evidence: `kdenlive`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=1.

## asset-point-read

C ABI: `pp_production_asset`, `pp_asset_set_count`, `pp_asset_set_get`, `pp_asset_set_release`

C++ projection: `Production::asset`

Python projection: `Production.asset`

Hosts with complete mechanical use evidence: `openassetio-manager`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=1.

## external-identifiers

C ABI: `pp_production_external_identifiers`, `pp_production_find_by_external_identifier`, `pp_external_identifier_set_count`, `pp_external_identifier_set_get`, `pp_external_identifier_set_release`, `pp_object_ref_set_count`, `pp_object_ref_set_get`, `pp_object_ref_set_release`, `pp_transaction_add_external_identifier`

C++ projection: `Production::externalIdentifiers`, `Production::findByExternalIdentifier`, `Transaction::addExternalIdentifier`

Python projection: `Production.external_identifiers`, `Production.objects_by_external_identifier`, `Transaction.add_external_identifier`

Hosts with complete mechanical use evidence: `blender`, `kdenlive`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=2.

## locator-inspection

C ABI: `pp_production_locators_page`, `pp_locator_query_set_count`, `pp_locator_query_set_get`, `pp_locator_query_set_next_cursor`, `pp_locator_query_set_release`

C++ projection: `Production::locators`

Python projection: `Production.locators_page`

Hosts with complete mechanical use evidence: none

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=0.

## resolution

C ABI: `pp_resolution_options_create`, `pp_resolution_options_add_search_directory`, `pp_resolution_options_release`, `pp_production_resolve_assets`, `pp_resolution_set_representation_count`, `pp_resolution_set_get_representation`, `pp_resolution_set_get_resource`, `pp_resolution_set_get_candidate`, `pp_resolution_set_get_candidate_evidence`, `pp_resolution_set_release`

C++ projection: `ResolutionOptions::addSearchDirectory`, `Production::resolveAssets`

Python projection: `Production.resolve`

Hosts with complete mechanical use evidence: `blender`, `kdenlive`, `openassetio-manager`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=3.

## revision-feed

C ABI: `pp_production_latest_revision`, `pp_production_changes_since`, `pp_revision_set_count`, `pp_revision_set_get`, `pp_revision_set_release`, `pp_production_revision_events`, `pp_revision_event_set_count`, `pp_revision_event_set_get`, `pp_revision_event_set_release`

C++ projection: `Production::latestRevision`, `Production::changesSince`, `Production::revisionEvents`

Python projection: `Production.latest_revision`, `Production.changes_since`, `Production.revision_events`

Hosts with complete mechanical use evidence: `kdenlive`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=1.

## content-verification

C ABI: `pp_production_verify_resource`

C++ projection: `Production::verifyResource`

Python projection: `Production.verify_resource`

Hosts with complete mechanical use evidence: `blender`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=1.

## host-object-binding

C ABI: `pp_host_binding_format`, `pp_host_binding_parse`, `pp_string_release`

C++ projection: `HostObjectBinding::toString`, `HostObjectBinding::fromString`

Python projection: `Production.host_bindings`, `Production.host_bindings.parse`

Hosts with complete mechanical use evidence: `openassetio-manager`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=1.

## cpp-result-propagation

C ABI: projection-only

C++ projection: `Result<T>`, `POSTPROJECT_TRY`, `POSTPROJECT_TRY_ASSIGN`

Python projection: none

Hosts with complete mechanical use evidence: `kdenlive`

Eligibility checks: existed at baseline=yes, unchanged after baseline=yes, independent hosts=1.
