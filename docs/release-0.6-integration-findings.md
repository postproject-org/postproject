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
