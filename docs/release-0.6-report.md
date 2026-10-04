# PostProject 0.6 candidate acceptance report

## Delivered candidate

Package `0.6.0-alpha.1` (Python `0.6.0a1`), C ABI **37**, **230 exported
symbols**, SQLite schema **17**. The released baseline is `v0.5.0-alpha.1`,
commit `c0c9a1a8a20c10f5c7401dda6758dce7f0ce2c58`, whose source, native and
Python artifacts were verified. This handoff prepares a pre-1.0 candidate;
no signed tag, package release or upstream outreach is performed.

Two complementary pilots consume installed public packages:

| Pilot | Host pin | Accepted route | Adapter revision |
|---|---|---|---|
| [OBS](https://github.com/postproject-org/postproject-obs) | 32.2.2 | C11 registration, C++17 frontend/worker plugin | `384fe5f86d06eba144f1353c95bf39d2ddf8409c` |
| [Natron](https://github.com/postproject-org/postproject-natron) | 2.5.0 | Native C++17 extension in embedded CPython, Reader/menu bridge | `3d4fa73cfadfb424e3c9fc6546b62101f4891ac8` |

Both repositories contain exact upstream pins, optional-dependency behavior,
fixtures, reproduction commands and fast installed-package CI. Their consumers
never invoke Cargo. The separate CI producer builds and installs PostProject.
Audacity 3.7.7 and Krita 5.2.13 were not selected under the minimum-pilot gate;
all four source-checked briefs and decisions are in the
{doc}`release-0.6-integration-findings`. Neither no-go is a failed build or a
claimed acceptance pass.

## Real-host acceptance

OBS ran under isolated Xvfb with real normal recording actions and a finalized
Matroska output. Seven success/fallback scenarios passed process exit and full FFmpeg
decode: registration, real-menu retry, unavailable production, close during
work, absent plugin, absent library and no chosen production. Independent
public readback found one asset and one capture activity; retry reused the
same facts. An eighth scenario killed the real muxer after it opened the
recording. This pinned OBS build still reported a successful stop, but output
validation rejected the aborted file; public readback found an empty production
without a revision. No playable-output claim is made for that failed recording.
Recording-validation diagnostics are distinct from database registration errors.
C contract regressions additionally passed failed staging/rollback,
repeated notification/lost acknowledgement and proxy-safe original readback.
Callback enqueue, worker, staging and commit timing remain separate.

Natron's real renderer process loaded the native C++ adapter and real
Reader/Writer nodes, rendered genuine PNG frames and exited successfully.
Normal acceptance preserved two explicitly adopted producer assets, unknown
metadata, canonical association and filename across save/reopen; it resolved
a named-root move, inspected two recorded locators, verified content and
refreshed durable revisions. Separate negative acceptance covered frame 1002
missing, two equally supported candidates, close/cancel, a changed filename,
copied values surviving result-owner destruction and a separate writer's
structured locator conflict. A third process reopened/rendered the fallback
without loading the optional native adapter. The Linux runtime archive SHA-256
is `1b423f983e89f6e3811fdabcf08caced3367ca24568dfdfe48a3a86c1ae95fa1`.

The incremental **OBS → Blender** handoff passed using Blender 5.2.2 LTS and
the maintained extension rebuilt with the candidate wheel. Blender's ordinary
save integration adopted OBS's original asset, retained capture provenance
and width/height metadata, resolved its moved recording through the normal
recovery action and saved/reopened the filename fallback with the extension
disabled. The receiving host read no OBS settings or project data.

The maintained **Kdenlive → Blender → OpenAssetIO Manager** shared-production
scenario also passed against the exact candidate library: explicit adoption,
render knowledge, cross-host locator observation, typed same-base conflict,
content replacement and derived-output staleness. Kdenlive 26.08.1 and Blender
5.2.2 full maintained suites passed; Kdenlive's runner now isolates configuration
and cache so the user's default profile cannot block automation.

This is automated real-host evidence. No interactive human pass, upstream
endorsement, full Natron rebuild, OpenFX project hook or second-platform pilot
acceptance is claimed.

## Interface and package verification

The native library was built in release mode from `6fe0088`, the version bump
plus lifetime repair; subsequent changes affect tools/docs/CI only. Its SHA-256
is `1ec0cb38e1f17acbba82d9b295a34d2ba2364c5e9dabfd636f88a96eed185c76`.
The authoritative headers and Python binding implementation are unchanged
from released 0.5. The closed-transaction guard defect is fixed by ADR 0044;
installed C and C++ tests cover releasing committed, rolled-back and
failed-commit handles while a newer transaction remains open. No schema,
ABI declaration or caller migration changed.

Local required gates passed: formatting, workspace/all-target/all-feature
Clippy with denied warnings, workspace/all-feature tests, no-dependency
rustdoc and cargo-deny. Installed C smoke, exact exported-symbol comparison,
compiler/ctypes layout comparison, generated Python signatures, C++ exception
and no-exception consumers, example coverage and strict documentation build
passed. The Python binding suite passed 44 tests with one documented skip;
the tool suite and focused family/dependency tests passed. OBS's actual C
adapter and both contract fixtures passed ASan/UBSan with leak detection;
the linked Rust library was not sanitizer-instrumented.

Maintained downstream suites passed against the candidate: OpenAssetIO,
OTIO/OpenAssetIO, Python host and OTIO demo (one each), maintained Manager
(six), and installed C++ NLE (one). Their package ranges include the 0.6
series; Manager is 0.3.0 and the demo accepts it. The C++ package's existing
SameMajorVersion rule accepts 0.6.
The two OTIO linker suites pass with an upstream unversioned-trait deprecation
warning; warning-as-error execution does not pass that upstream dependency.
The same producer/installed-consumer jobs are retained on Linux, macOS and
Windows. [Candidate core CI](https://github.com/postproject-org/postproject/actions/runs/37194261178)
passed all 16 jobs at `d133613`, including native packages and tests on all three
platforms, strict documentation, MSRV, sanitizers and the glibc 2.28 wheel.
Subsequent changes to this candidate concern evidence/docs only; native source,
headers and binding implementation remain identical to that run.

The ten downstream acceptance jobs have completed passing evidence in
{download}`the CI record <evidence/release-0.6/downstream-ci.json>`:
Ardour's unchanged Linux builds with/without PostProject and macOS resolver
come from the original candidate run; the other consumers come from the
[corrected run](https://github.com/postproject-org/postproject/actions/runs/37202457126).
The original aggregate failed because Python host CI collected the nested
producer's tests. Commit `98e7ee1` scopes collection to its own host module,
which now passes. The corrected run's redundant full Ardour builds were
still running when this evidence snapshot was taken; their pending status
is not represented as another pass. All jobs use the same candidate native
source. {download}`Core CI metadata <evidence/release-0.6/core-ci.json>`
retains the exact source commit and sixteen successful jobs.

No hard pilot latency gate is introduced. Existing performance policy has
no calibrated hard release threshold. Actual OBS callback/shutdown and Natron
lookup-scaling observations are recorded in the findings and trace metadata.
Normal/failure correctness checks have deterministic status assertions,
independent of measured durations.

## Compatibility evidence and reproduction

The {doc}`release-0.6-compatibility-evidence` reuses the existing offline tool.
Complete per-language projection use and acyclic family dependency closure
are enforced by tested tooling. The candidate matrix includes whole known-media
lookup and base-revision conflict families without splitting them to qualify.
The {doc}`release-0.6-compatibility-decision` proposes only source-level C++
Result propagation, supported by Kdenlive and Natron. **Maintainer approval is
pending; no new promise is enacted.** The changed transaction family cannot
qualify, and useful read-family closures retain the explicitly described
Manager test-host independence limitation. Failure/contract traces are retained
separately and do not manufacture normal-path host counts.

Trace inputs and adapter commits are recorded in
{download}`the provenance manifest <evidence/release-0.6/manifest.json>`.
The baseline traces originate from accepted runs 36847303316, 36848109149 and
36838046823. The C++ projection files record compiled source/helper use,
separately from ABI calls. Regenerate the candidate report offline from the
repository root:

```sh
python tools/compatibility_usage.py \
  --trace kdenlive=docs/evidence/release-0.6/kdenlive.txt \
  --trace blender=docs/evidence/release-0.6/blender.txt \
  --trace openassetio-manager=docs/evidence/release-0.6/openassetio-manager.txt \
  --trace obs=docs/evidence/release-0.6/obs-normal.txt \
  --trace natron=docs/evidence/release-0.6/natron-normal.txt \
  --projection kdenlive=docs/evidence/release-0.6/kdenlive-cpp.txt \
  --projection natron=docs/evidence/release-0.6/natron-cpp.txt \
  --propose cpp-result-propagation \
  --output docs/release-0.6-compatibility-evidence.md
```

No previously compiled cross-series substitution is claimed: 0.5 named no
such promise. Source compatibility, ABI mechanics, host semantics and
maintainer approval remain separate evidence.

## Prepared artifacts and limitations

Local review artifacts are a native Linux x86_64 install-prefix archive
(including CLI, shared/static libraries, headers, CMake/pkg-config metadata,
licenses and installed quickstarts), pure/platform Python wheels, source
archive and SHA-256 manifest. The local Linux platform wheel is tagged
`linux_x86_64`; it is not asserted to meet manylinux glibc 2.28. Existing CI
retains the dedicated glibc 2.28 wheel build and all three native platforms.
Artifacts are candidate builds, not published releases.
Extracted native C/C++ quickstarts passed without Cargo. The platform-wheel
quickstart passed in a fresh environment without library search variables,
and its embedded library hash matches the real-host evidence.

Support remains a local SQLite production on one machine. Network storage,
authentication and distributed collaboration are outside scope. OBS supports
one simple local Matroska recording, with volatile attempt identity and bounded
explicit retry, rather than crash-safe exactly-once delivery. An in-flight
hash or commit can delay joined shutdown. The isolated OBS runs also emit a
host teardown callback warning after the adapter's verified join boundary;
all accepted runs exit zero and preserve decoded media. Natron acceptance uses
the pinned Linux renderer route; its GUI menu is supplied but not claimed as
a human-tested graphical session. No second job renew/fail/cancel consumer,
render scheduler, audio selection mapping or universal host model is delivered.
