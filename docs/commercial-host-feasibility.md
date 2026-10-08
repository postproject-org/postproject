# Commercial host feasibility

These are implementation briefs for integrators, based on the named vendor
documentation. Neither REAPER nor Resolve has an executed PostProject adapter.
The [Nuke preparation](https://github.com/postproject-org/postproject-openassetio-manager/blob/main/docs/nuke-trial.md)
uses the existing OpenAssetIO Manager and records its separate runtime status.

## REAPER 7.82

The reviewed [API reference](https://www.reaper.fm/sdk/reascript/reascripthelp.html)
identifies REAPER 7.82. Its APIs support compiled C/C++ extensions and ReaScript.
Lua and EEL2 are embedded; Python requires separate setup. The published reference
can change, so an installed build must supply the matching API evidence.

Use a small C++17 extension consuming the installed PostProject CMake package.
This costs a native build and packaging per supported platform. A Lua action
calling the CLI costs less initially, but needs an explicit process/file error
contract. Python adds interpreter configuration. The
[Extension SDK](https://www.reaper.fm/sdk/plugin/plugin.php) and
[ReaScript guide](https://www.reaper.fm/sdk/reascript/reascript.php) describe these
entry points.

`SetProjExtState` / `GetProjExtState` provide project persistence. Take and item
`GetSet…Info_String` APIs expose persistent `P_EXT:xyz` data and GUIDs. Store the
production UUID and reference-location hint in project state, and canonical
representation references on takes. An item is an edit occurrence; its take's
source identifies media. A copied take may legitimately share that representation
while needing a separate host association. Host GUIDs remain external identities.

Declare one action: **Adopt selected take**. Read its source, find known media,
ask the user to choose when ambiguous, then commit and attach the reference.
Copy values for workers; recheck the active project/take before applying results.
Keep filesystem inspection outside audio processing callbacks. Native source
replacement must follow the documented `GetSetMediaItemTakeInfo(P_SOURCE)`
ownership contract; `SetMediaItemTake_Source` is discouraged for C/C++ callers.

Acceptance requires saved project reopen, Save As, item/take duplication and
cross-project copy to preserve the intended references without aliasing host
occurrences. Missing production/media and rejected commits must leave ordinary
playback usable. Uninstall leaves native media paths and harmless persisted
references; any relink updates concrete paths only after explicit confirmation.
Unproven persistence or source ownership blocks relinking. Track/mix exchange and
automatic stem publication are outside this action.

## DaVinci Resolve 21.1.1, free Linux edition

Reviewed the supplied `DaVinci_Resolve_21.1.1_Linux.zip`, its installation/release
notes and bundled `Developer/Scripting/{README.md,DaVinciResolveScript.pyi}`.
The [source record](commercial-host-sources.json) pins archive and document
checksums. Documentation was extracted without running the installer. Resolve
runtime/build-number inspection, scripting access and persistence were not
executed; no Studio entitlement is assumed.

The vendor documents a bundled 64-bit Python 3.14 interpreter at
`/opt/resolve/bin/ResolvePython`, with the scripting module available directly
and no pip support. Internal Console/menu scripts have a `resolve` global;
user scripts belong under
`~/.local/share/DaVinciResolve/Fusion/Scripts/Utility`. External scripting
configuration is documented under Studio. Free-edition internal access and each
required method must be checked in the actual application; presence in the
shared API definitions is insufficient.
The supplied reference manual, chapter 4, page 106, independently marks external
scripting as Studio-only.

The supplied transcript of the [21.1 scripting discussion](https://forum.blackmagicdesign.com/viewtopic.php?f=44&t=239905)
quotes release notes moving Python scripting to Studio. It also reports removal
of Lua FFI from the free edition; the exact remaining Lua API set is unspecified.
The forum blocked automated retrieval, so this evidence came from the user,
separately from the bundled API definitions. Plan the Python route for **Studio**;
the downloaded free distribution does not qualify it.

For Studio, use a user-invoked Python action consuming the installed PostProject
binding and matching native library, deployed to a deliberate import location.
Validate loading against the host interpreter; do not assume pip or the
development environment is available inside Resolve. A free-edition Lua action
with a CLI helper is only a conditional route: first verify its host APIs and
permitted process/file operations. It cannot rely on Lua FFI. If those checks
fail, the fallback is manual media export plus a separately invoked PostProject
CLI, without automatic host reference persistence. No native Resolve extension
or new public JavaScript binding is needed for the proposed route.

`Project.GetUniqueId`, `MediaPoolItem.GetUniqueId` and `GetMediaId` expose host
identities, but the vendor descriptions do not promise their stability across
project export/import or duplication. Keep them as opaque external values.
Proposed persistence is per-clip `SetThirdPartyMetadata` / `GetThirdPartyMetadata`
with namespaced keys containing production and canonical representation
references. Test save/close/`LoadProject`, Save As, DRP export/import, duplicate
clips and subclips before relying on this storage. Read back exact strings and
preserve unrelated metadata. No marker or filename fallback silently becomes
canonical identity when metadata persistence fails.

Declare **Register selected media-pool clips** as the initial action. Resolve
known media first, require explicit selection on ambiguity, commit PostProject,
then persist references and report partial host-write failure without inventing
an atomic transaction across applications. Relinking remains separately
user-invoked: `MediaPool.RelinkClips` changes folders, while `ReplaceClip` replaces
the underlying asset and metadata. Neither is assumed to preserve references or
subclip extents without testing. Render-output registration may inspect a named
job through `GetRenderJobList` / `GetRenderJobStatus` and register verified output
after completion; it does not acquire a PostProject worker lease automatically.

Respect Resolve's project libraries and collaboration. `RefreshFolders` and
`GetIsFolderStale` expose collaboration refresh state, without providing a
cross-application lock or transaction. Reacquire host objects and check identity
before writing; use PostProject's normal bases/conflicts. Project-library SQL is
outside this integration. Uninstall retains concrete media paths, ordinary
project operation and inert third-party references; missing production or
unsupported scripting leaves registration/relink unavailable.

The release notes require Rocky Linux 8.6, 32 GB RAM, a discrete GPU with 4 GB
VRAM and OpenCL 1.2 or CUDA 12.8; the named NVIDIA driver minimum is 580.119.02.
The free edition's processing/output limits and codec availability apply. A
working GPU/runtime, the chosen edition's scripting access and demonstrated
persistence are acceptance conditions, not evidence supplied by this archive.
Studio entitlement and runtime remain unverified.

The supplied community knowledge base v5.7.1 covers API 21.1 and separately
labels runtime reports for 19.1.4 / 21.0.4. Its version section also marks Python
as Studio-only. Use it as a source of test cases; its API summaries and older
runtime reports do not qualify this 21.1.1 distribution. In particular, verify
relink results by rereading `GetClipProperty("File Path")` and the canonical
reference. Reported partial setter behavior reinforces the need for readback
after host writes. Record `GetVersion`, `GetProductName` and `IsStudio` in actual
acceptance evidence; those methods are present in the vendor definitions.
