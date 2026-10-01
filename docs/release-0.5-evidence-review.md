# PostProject 0.5 evidence review

This review applies the compatibility-family evidence gate after the maintained
Kdenlive, Blender, and OpenAssetIO Manager shared-production scenario. The
input is their full acceptance-suite traces unioned with their isolated
per-process shared-scenario traces on 2026-10-01. The generated
{doc}`release-0.5-compatibility-evidence` report records every required C
operation and the all-or-nothing family result.

The release audit regenerated that report from the successful CI artifacts for
[Kdenlive and the shared scenario](https://github.com/postproject-org/postproject-kdenlive/actions/runs/36847303316),
[Blender 5.2 and 5.3](https://github.com/postproject-org/postproject-blender/actions/runs/36848109149),
and the [OpenAssetIO Manager](https://github.com/postproject-org/postproject-openassetio-manager/actions/runs/36838046823).
The per-host unions and Kdenlive projection evidence reproduce the committed
matrix byte-for-byte.

## Family evidence

Two families have complete normal-path ABI evidence from at least two
independent hosts and remained unchanged across the 0.4-to-0.5 boundary:

- `external-identifiers`: Blender and Kdenlive;
- `resolution`: Blender, Kdenlive, and the OpenAssetIO Manager.

The single-consumer or unused families are:

- `production-lifecycle`: OpenAssetIO Manager only;
- `transaction-lifecycle`: Kdenlive only;
- `asset-point-read`: OpenAssetIO Manager only;
- `locator-inspection`: no complete host trace;
- `revision-feed`: Kdenlive only;
- `content-verification`: Blender only;
- `host-object-binding`: OpenAssetIO Manager only;
- `cpp-result-propagation`: Kdenlive only.

This is evidence, not a compatibility promise. The release decision still
requires applying the complete semantic and projection criteria and maintainer
approval. The {doc}`release-0.5-compatibility-decision` records the accepted
no-subset outcome: neither eligible leaf family has a qualified foundational
dependency closure.

## Coverage questions

1. **Families with one host:** production lifecycle, transaction lifecycle,
   asset point-read, revision feed, content verification, host-object binding,
   and C++ result propagation. Locator inspection has no complete host.
2. **Projection gaps:** only Kdenlive supplies real-host C++ projection
   evidence. Blender and the Manager exercise Python. Direct C remains limited
   to repository consumers and smoke tests rather than a maintained host.
3. **`pkg-config`:** the selected Ardour resolver now compiles its executable
   policy through the installed file without pilot-specific package changes.
4. **Exception-oriented C++:** the Ardour boundary uses `Result<T>::value()`
   and catches `postproject::Exception` in the application's normal
   exception-enabled build. Its executable scenario verifies the typed throw.
5. **Non-Linux route:** the Ardour pilot's macOS installed-package resolver
   scenario passed in the
   [maintained downstream workflow](https://github.com/postproject-org/postproject-ardour/actions/runs/36840953154).
   This verifies the resolver and package route, not an interactive macOS
   Ardour session. Core native packages and examples also run there.
6. **Job terminal paths:** Kdenlive exercises a real proxy worker and the
   OpenAssetIO Manager completes publishes, but a second host does not exercise
   renew, fail, and cancel together. Those paths are not members of a proposed
   0.5 compatibility family.
7. **Raw C:** no maintained application host uses it directly. Repository C
   examples and the installed-package smoke consumer cover ownership and ABI
   mechanics.
8. **Architecture:** the shared scenario exposed no missing domain concept or
   storage design problem. Explicit adoption, origin, revision reads, base
   revisions, semantic conflicts, provenance snapshots, and artifact
   evaluation represented the workflow.

## Pilot decision and outcome

The review selected the Ardour resolver pilot only. The maintained patch series
and executable Linux scenario now exercise the installed `postproject.pc`, a
second C++ desktop application's exception boundary, and audio-domain media.
The exact evidence and its limits are recorded in
{doc}`release-0.5-ardour-pilot`.

Shotcut was not selected. Its strongest unique additions were a Windows host
and a second complete proxy-worker lifecycle. Neither was required to decide
the two eligible families, and the shared-production scenario exposed no
job-model uncertainty requiring another editor patch.

The GES/Pitivi raw-C experiment remained closed. The maintained application
hook is a Python plugin; a manufactured native shim would test PostProject
rather than a normal host integration. Repository C consumers remain the
proportionate coverage for this release.
