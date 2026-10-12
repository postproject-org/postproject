# Try an integration or demonstration

These are maintained experimental pilots. Use matching SDK and integration
versions, and read the linked installation guide before starting. The 0.9 SDK
and corresponding packages are prepared development sources, not published
downloads. Current Linux host evidence is automated; interactive human
acceptance and upstream endorsement are not claimed.

| Application or demo | Useful first task | Evaluation route and current scope |
|---|---|---|
| [Blender](https://github.com/postproject-org/postproject-blender/blob/main/ADOPTION.md) | Recover renamed Sequencer media by content | Prepared 0.3.0 extension, matching SDK wheel included; Blender 5.2.2/Linux installed-host checks pass |
| [Kdenlive](https://github.com/postproject-org/postproject-kdenlive/blob/main/ADOPTION.md) | Record clips and recover missing media | Optional source patches for 26.08.1; full Linux builds/tests pass with and without the SDK |
| [OBS recording pilot](https://github.com/postproject-org/postproject-obs) | Register a finalized recording's identity and capture facts | Installed C SDK and optional frontend plugin; eight real OBS 32.2.2/Linux scenarios pass |
| [Natron](https://github.com/postproject-org/postproject-natron) | Resolve a compact image sequence into an ordinary Reader | Native source module for 2.5.0; normal/negative renders and plugin-free fallback pass on Linux |
| [Ardour](https://github.com/postproject-org/postproject-ardour) | Recover renamed audio by content | Optional source patches for pinned 9.8; installed resolver passes; full current builds await CI dependencies |
| [OpenAssetIO Manager](https://github.com/postproject-org/postproject-openassetio-manager) | Resolve a recorded entity reference through a standard host interface | Manager 0.6.0 with SDK 0.9.0a1; isolated installed-package tests pass |
| [OTIO demo](https://github.com/postproject-org/postproject-otio-demo) | Resolve a timeline's references through the Manager | Demo 0.4.0 with the matching Manager/SDK; installed demo passes |
| [Nuke trial](https://github.com/postproject-org/postproject-openassetio-manager/blob/main/docs/nuke-trial.md) | Save and relocate one managed Read sequence | Nuke 17.0v1 preparation passes; licensed host execution is not executed |

Other desktop platforms are source/build routes until their current installed
SDK and pilot jobs pass. A wheel tag alone does not establish host support.
Use each pilot repository's issues for experimental support and its stated
removal or disabled-plugin fallback.

## OBS → Blender

OBS records the finalized media, stable identity, technical metadata and capture
activity in one production. In Blender preferences, choose that same `.pproj`,
add the recording to the Sequencer, and save. Blender adopts the existing
identity when its locator has one complete match. Move the recording and use
**Find Missing Media by Content**; save the accepted path. The automated
walkthrough also reopens the `.blend` with the extension disabled.

Run the recording pilot's `tests/run.py`, then its `tests/handoff_blender.py`
in background Blender with the built extension. Its README gives the exact
arguments. Media files travel separately from the production file.

## Kdenlive → Blender → OpenAssetIO Manager

Select one explicit production in Kdenlive and Blender. Kdenlive records the
original clip; Blender adopts it and records a completed still render with its
inputs and producing activity. The configured Manager resolves the recorded
result on this machine. Kdenlive's
[`tools/shared-production.sh`](https://github.com/postproject-org/postproject-kdenlive/blob/main/tools/shared-production.sh)
is the runnable walkthrough: it also covers moved media, a competing-write
conflict and changed-source staleness.

These are two separate qualified handoffs. They do not establish a combined
three-application OBS recording workflow, timeline conversion or automatic
cross-machine synchronization. Each host retains its own project, and a local
mapping determines where this machine finds media.
