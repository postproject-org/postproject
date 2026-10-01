# PostProject 0.5 evidence review

This review applies the compatibility-family evidence gate after the maintained
Kdenlive, Blender, and OpenAssetIO Manager shared-production scenario. The
input is their full acceptance-suite traces unioned with their isolated
per-process shared-scenario traces on 2026-10-01. The generated
{doc}`release-0.5-compatibility-evidence` report records every required C
operation and the all-or-nothing family result.

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
approval.

## Coverage questions

1. **Families with one host:** production lifecycle, transaction lifecycle,
   asset point-read, revision feed, content verification, host-object binding,
   and C++ result propagation. Locator inspection has no complete host.
2. **Projection gaps:** only Kdenlive supplies real-host C++ projection
   evidence. Blender and the Manager exercise Python. Direct C remains limited
   to repository consumers and smoke tests rather than a maintained host.
3. **`pkg-config`:** no maintained host uses the installed file.
4. **Exception-oriented C++:** no maintained evidence specifically validates
   the wrapper in an application's normal exception-based error style.
5. **Non-Linux host:** no maintained real-host scenario runs on macOS or
   Windows. Core native packages and examples do run there.
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

## Minimum pilot decision

Select the Ardour resolver pilot only. Its verified seam can add three evidence
routes that remain materially absent: the installed `postproject.pc`, a second
C++ desktop application using normal exceptions, and an audio-domain host. A
macOS compile is included if the host build remains practical; a Linux resolver
scenario remains the behavioral acceptance path.

Do not select Shotcut. Its strongest unique additions are a Windows host and a
second complete proxy-worker lifecycle. Neither is required to decide the two
currently eligible families, and the shared-production scenario revealed no
job-model uncertainty that justifies another editor patch.

Keep the GES/Pitivi raw-C experiment closed. The maintained application hook is
a Python plugin; manufacturing a native shim would test PostProject rather than
a normal host integration. Repository C consumers remain the proportionate
coverage for this release.
