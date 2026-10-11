# PostProject 0.8 compatibility evidence

Baseline: `0.7.0-alpha.1`. Candidate: `0.8.0-alpha.1`.

| Family | Last change | ABI | blender | kdenlive | natron | obs | openassetio-manager | Mechanically eligible | Eligible with dependencies |
|---|---|---:|---|---|---|---|---|---|---|
| `production-lifecycle` | `0.8.0-alpha.1` | 52 | — | ABI | — | ABI | — | no | no |
| `transaction-lifecycle` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `asset-point-read` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `external-identifiers` | `0.8.0-alpha.1` | 52 | ABI | ABI | — | — | — | no | no |
| `locator-inspection` | `0.7.0-alpha.1` | 51 | — | — | ABI | — | — | no | no |
| `resolution` | `0.7.0-alpha.1` | 51 | ABI | — | — | — | — | no | no |
| `revision-feed` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `content-verification` | `0.7.0-alpha.1` | 51 | ABI | — | ABI | — | — | yes | no |
| `host-object-binding` | `0.7.0-alpha.1` | 51 | — | — | ABI | — | ABI | yes | yes |
| `cpp-result-propagation` | `0.4.0-alpha.1` | 32 | — | projection | projection | — | — | yes | yes |
| `known-media-lookup` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `base-revision-conflicts` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `asset-identity` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `media-root-identity` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `locator-identity` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `locator-retirement` | `0.8.0-alpha.1` | 52 | — | ABI | — | — | — | no | no |
| `job-identity` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `activity-identity` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `representation-identity` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `resource-identity` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `external-identifier-removal` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `dependency-observation` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `media-root-pages` | `0.7.0-alpha.1` | 51 | — | — | — | — | — | no | no |
| `coherent-read-edit` | `0.8.0-alpha.1` | 52 | ABI | ABI | ABI | — | — | no | no |
| `detached-decision-edit` | `0.8.0-alpha.1` | 52 | — | ABI | — | — | — | no | no |
| `job-lease-completion` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `job-lease-renewal-failure-cancellation` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `job-lease-private-transport` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `exchange-inspection` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `proposal-preparation` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `submission-outcome-recovery` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `proposal-submission-delivery` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `checkpoint-exchange` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `change-stream-exchange` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |
| `authority-history-resynchronization` | `0.8.0-alpha.1` | 52 | — | — | — | — | — | no | no |


The table is generated from the retained installed-candidate traces. Symbol
reachability establishes complete mechanical family use, not branch coverage or
compatibility promises. Content verification is mechanically eligible but its
lifecycle dependencies are not. Host-object binding and C++ Result propagation
have complete mechanical evidence with their required families.

No additional stable family is named for 0.8. The existing 0.6.x C++ Result
promise remains intact; neither these traces nor rebuilt consumers establish
cross-series binary replacement. Families changed by passive edit guards are
marked as changed in 0.8 at ABI 52. Exchange families were absent from the 0.7
baseline and remain experimental.

Kdenlive and Blender inputs cover the maintained shared-production path,
including explicit conflict and staleness checks. The Manager is one test-host
route. OBS and Natron inputs cover their normal real-host paths; negative and
contract runs supply separate correctness evidence. Source-checked C++ tokens
come from the compiled Kdenlive and Natron adapters. The separate exchange
fixture helpers do not add independent applications. A second real consumer of
renew/fail/cancel remains absent.

{download}`Input provenance <evidence/release-0.8/compatibility-inputs.json>`
records SDK/adapter revisions, installed artifacts, scope and trace digests.
Reproduce the matrix and detailed dependency review:

```sh
git show a5417522789909489c6fe4085f02e84a49c9f786:docs/compatibility-families.toml > target/release-0.8-families.toml
python3 tools/compatibility_usage.py \
  --manifest target/release-0.8-families.toml \
  --trace kdenlive=docs/evidence/release-0.8/kdenlive.txt \
  --trace blender=docs/evidence/release-0.8/blender.txt \
  --trace openassetio-manager=docs/evidence/release-0.8/openassetio-manager.txt \
  --trace obs=docs/evidence/release-0.8/obs-normal.txt \
  --trace natron=docs/evidence/release-0.8/natron-normal.txt \
  --projection kdenlive=docs/evidence/release-0.8/kdenlive-cpp.txt \
  --projection natron=docs/evidence/release-0.8/natron-cpp.txt
```
