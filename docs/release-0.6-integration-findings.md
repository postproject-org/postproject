# Release 0.6 integration findings

## Baseline and scope

The inspected and remotely verified baseline is `v0.5.0-alpha.1`, commit
`c0c9a1a8a20c10f5c7401dda6758dce7f0ce2c58`: package `0.5.0-alpha.1`,
C ABI 37 (230 symbols), schema 17. The tag has published Linux, macOS,
Windows, source, and Python artifacts. Its accepted policy names no
compatibility subset. The development candidate is `0.6.0-alpha.1`.

The automated Kdenlive–Blender–Manager scenario and Ardour's installed
resolver evidence are retained as recorded in the {doc}`release-0.5-report`.
They do not imply an interactive graphical pass. Shotcut was not selected;
GES/Pitivi's Python application hook did not supply the native evidence sought.

## Candidate briefs and selection

### OBS Studio — selected

Pin: OBS `32.2.2`, commit `ba2f32bdf791005443988a4955e963663e16b1ed`.
Source: `frontend/widgets/OBSBasic_Recording.cpp`,
`frontend/utility/BasicOutputHandler.cpp`, `frontend/OBSStudioAPI.cpp`,
`libobs/obs-output.c`, and `plugins/obs-ffmpeg/obs-ffmpeg-mux.c`.

The frontend stopped event is emitted even after output failure. The raw
recording output's `stop` signal supplies the actual status code. An optional
frontend plugin copies that signal's bounded data and processes a single
ordinary Matroska recording on its own worker. PostProject calls and handle
cleanup belong to a C11 translation unit; a thin Qt/C++ shim handles the host
menu, queue, probing, and shutdown. Package discovery uses installed
`postproject.pc`. No processing callback hashes or queries a production.

The user selects or creates a production outside capture. Normal-path targets
are production/transaction lifecycle and asset readback; failed staging rolls
back explicitly. Recording attempt identity lasts for the plugin session and
explicit retry, including lost acknowledgement. It is not global deduplication
or crash-safe exactly-once delivery. Missing plugin/library/production preserves
ordinary recording. Stop status, playable output, registration failure,
duplicate notification, and unload during work are the acceptance cases.
Split recording, replay, remux, and streaming remain outside the supported slice.

This supplies the missing genuine C construction and cleanup consumer. A
missing completion status or inability to quiesce callbacks would invalidate
this route; neither is assumed from the frontend event documentation alone.

### Natron — selected

Pin: Natron `2.5.0`, commit `53ee17e34438005425230468916ff223cfd8a403`.
Source: `Engine/AppManager.cpp`, `Engine/PyNode.cpp`,
`Engine/PyParameter.cpp`, `Engine/PyAppInstance.cpp`, and `Gui/PyGuiApp.cpp`.
The official Linux package embeds CPython 3.10 and Qt 4. Native CPython
extensions are loaded in the host's interpreter; no OpenFX project hook is
assumed. The adapter uses a compiled C++17 extension for all PostProject
operations, with a small Python menu/Reader bridge. This route tests C++
Result propagation in the actual host; it does not use the PostProject Python
binding or claim raw-C ergonomic evidence.

The action associates the selected Reader with an explicitly selected
production/sequence candidate. A persistent user string parameter stores the
canonical binding while the native filename/pattern remains the fallback.
Reader creation, parameter persistence, save/reopen, and revision refresh use
the host API. The native extension uses installed CMake packages and copies
results into host-independent values before result owners are released.

Targets: naming-aware adoption, compact sequence inspection, verification,
host binding, revision feed, and all three C++ propagation helpers. Fixtures
have genuine PNG frames, start at 1001, use four-digit padding, and include two
patterns in one directory. Acceptance includes a moved directory, missing
required frame, ambiguity, save/reopen without the production, and discarding
an obsolete asynchronous result. Ordinary Reader behavior survives removal of
the optional extension. This gives native sequence consumption distinct from
Blender's Python route. A native extension that cannot load in the pinned
interpreter is a blocker, not a successful Python fallback.

### Audacity — not selected

Pin: Audacity `3.7.7`, commit `5ef610ed23260d6d648175735bb16b32536eb30b`.
Source: `modules/import-export/mod-pcm/ExportPCM.cpp`,
`libraries/lib-module-manager/ModuleManager.cpp`,
`src/ProjectFileManager.cpp`. PCM export receives project selection times as
double seconds and output sample rate/channels separately. Completion checks
write and file-close outcomes. A native patch/module must match the host build;
script-pipe registration would not test native C++ consumption.

A bounded WAV import/selection export could record approximate project times
and actual output sample facts in existing metadata and activities. AUP3
storage is private project data, not a collection of relinkable WAV files.
44.1/48 kHz, mono/stereo, unknown layout, and non-video-aligned selections are
the appropriate fixtures; no exact original-source range can be inferred
after editing/resampling. Export failure and registration failure are separate.

Ardour already supplies installed exception-enabled C++ and genuine audio
resolver evidence. OBS covers new construction/metadata/capture use, while
Natron covers native projection and compound reads. Selected-range audio
mapping would add host facts, but no remaining declared family or package
gap requires it. Under the minimum-pilot gate, that does not justify another
matched native host build. This is a redundancy decision, not a passed export
test or a claim that an Audacity build failed.

### Krita — not selected

Pin: Krita `5.2.13`, commit `6d3651ac4df88efb68e013d21061de9846e83fe8`.
Source: `libs/libkis/Document.cpp`, `libs/libkis/Document.h`, and
`plugins/extensions/pykrita`. `modified()`, `waitForDone()`, and `exportImage()`
provide a bounded saved-document PNG export route. A plugin must require a
saved, unchanged input and respect export completion before registration.
The source's export path calls `exportDocumentSync` and returns its outcome.

Fixtures would use a saved `.kra`, one PNG, unknown metadata, failed export,
missing PostProject, and explicit source observation followed by staleness.
The maintained Python projection already has Blender's saved-input PNG render
and provenance consumer, and the selected handoff is OBS to Blender. Krita
would repeat that projection/export boundary without closing a residual native
or foundational family gap. No Krita adapter or acceptance pass is claimed.

## Transaction guard defect

The public C regression failed against the installed 0.5 library for commit,
rollback, and failed commit. Releasing a closed transaction A after beginning
B cleared B's guard. The C++ move/destruction regression failed on the same
sequence. ADR 0044 fixes destruction to clear only an open transaction's guard.
All installed regressions pass against the candidate. Declarations, ABI 37,
schema 17, and projection ownership remain unchanged; no migration is needed.
The transaction family's last-change release is conservatively recorded as
0.6 because its observed lifecycle behavior changed.

## Evidence repairs

Projection-only evidence now requires all members of one language family per
host. Family dependencies have validated acyclic closure and deterministic
proposal output. Tests cover incomplete Result helpers and the historical
identifiers/resolution-without-foundations case. Historical 0.5 reports are
unchanged. Mechanical eligibility does not establish maintainer approval.

## OBS readback and shutdown

The initial C recipe assumed representation row zero was the imported
original. A public-API fixture added a proxy between media import and capture
registration and reproduced capture provenance attached to that proxy.
Representation ordering does not express original identity. The adapter now
examines a bounded set, selects a unique original single-resource representation
and refuses ambiguity. The C regression verifies activity outputs on both
original and proxy. This is host policy; no shared API or schema changed.

The successful output callback copies bounded facts only. One joined worker
owns probe/hash/staging/commit and cleanup, with one pending attempt and explicit
retry of the latest retained attempt. Accepted failure and shutdown runs keep
fully decodable recordings. Already running fingerprint I/O remains a shutdown
latency limitation. The final normal run measured 0.024 ms callback enqueue,
140.468 ms worker elapsed, 1.949 ms staging and 2.713 ms commit calls. Close during
work measured 0.021 ms enqueue and 128.722 ms worker elapsed before cancellation
and join. These tiny fixtures provide informational observations, not budgets.

A separate real recording-failure fixture kills the test host's own muxer
after it opens its output. The pinned OBS build reports a successful stop even
for this aborted, empty file. The adapter's independent output validation
rejects it before any import, leaving an empty production without a revision.
Recording-validation and database-registration failures have distinct
diagnostics. This reproduces why completion status alone is insufficient;
no playable-output claim or upstream repair is made for that failure.

## Natron decisions and conflicts

The pilot's [own brief](https://github.com/postproject-org/postproject-natron/blob/main/BRIEF.md)
and the [OBS brief](https://github.com/postproject-org/postproject-obs/blob/main/BRIEF.md)
retain source seams and acceptance boundaries beside their adapters.

The pinned host loads the native C++ extension in embedded CPython 3.10.
Its two genuine padded PNG sequences share a directory; explicit adoption
preserves both producer identities and unknown metadata. Named-root movement,
verification, bounded recorded-locator display, revision refresh, save/reopen,
and dependency-free filename fallback pass through real Reader/Writer runs.
The bridge never substitutes PostProject's Python binding for native calls.

A decision reads its base before the facts and checks the revision again after
resolution. Host object, association and filename checks discard obsolete
asynchronous results before application. Already recorded locations are
read-only no-ops; new confirmation uses the retained base. An independent
public-API writer invalidates the old locator decision: structured conflict
information reaches the bridge, the transaction closes, and the Reader keeps
its fallback. Partial frames, ambiguous candidates, close/cancel and copied
result values have separate negative evidence.

The candidate audit found that the bridge tested whether a script name existed,
then dereferenced its retained Reader wrapper. Pinned `Engine/PyNodeGroup.cpp`
returns a fresh wrapper on lookup, while `Engine/PyNode.cpp` retains a weak
native reference. Names can be reused. Deterministic tests reproduced stale
wrapper access and applying an older request after a newer one began.
The bridge now uses a nonpersistent generation parameter and looks up the
current Reader after native work completes. Three regressions and a real-host
replacement-name case pass, alongside the complete normal, negative and
dependency-free fallback renderer acceptance. This is an adapter lifetime
repair; no public PostProject contract or family last-change date changes.

The guide's former read-then-base recipe could attach a new base to stale facts.
The corrected recipe uses a before/after revision fence, documents empty
productions without a base, and never calls a later latest revision its own
commit receipt. This is a documentation repair to existing contracts.

The actual native lookup path averaged 2.452 ms for 100 queries with two assets
and 2.727 ms with 202 assets. This single Linux observation tests the real
naming-aware path and is informational. It does not calibrate a hard CI limit.

## Usability, collaboration and standards review

The {doc}`src/integrators/native-host-actions` links runnable installed C/C++
recipes for ordinary success, partial acquisition cleanup, terminal transactions,
retained decisions, no-op confirmation, ownership and stale/conflict handling.
C owns every handle and copies bounded outputs; C++ uses owning values and all
Result propagation helpers. No host allocator releases native memory.

Both pilots retain canonical production/object identity and ordinary host
filename fallback. Origin is application/version, not authenticated person
identity. Mutations remain explicit and atomic; durable revisions support
refresh without pretending to observe file changes automatically. OBS's two
commits are independently retryable facts, not a claimed atomic capture-plus-
import transaction. Sequence frame number, playback rate and naming remain
separate. Missing/ambiguous media is never silently remapped.

The C header, C++ wrapper and Python implementation remain byte-for-byte
unchanged from released 0.5. No new domain primitive was necessary. Under the
standards policy, local cleanup, host scheduling, existing binding strings,
qualified host identifiers and observation activities add no normative
standards mapping. Unknown metadata remains verbatim. ADR 0044 records the
shared lifetime correction; the evidence tool's clarification stays in ADR 0040.

No render worker extension was selected. Natron's bounded Reader path supplies
no trustworthy job renew/fail/cancel lifecycle, and neither pilot claims job
coverage. Audacity and Krita remain documented no-go decisions; neither is
represented as an accepted host workflow.
