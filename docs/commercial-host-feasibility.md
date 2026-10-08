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
