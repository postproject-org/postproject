# PostProject 0.5 integration findings

This document records evidence from maintained integrations against the 0.5
development series. It describes completed observations and release-scoped
limitations; it does not assign work to a later release.

## Shared local production

On 2026-10-01, the installed Kdenlive pilot, installed Blender extension, and
OpenAssetIO Manager completed the automated Linux scenario against one explicit
temporary `.pproj` path:

1. Kdenlive recorded a camera source and proxy with Kdenlive revision origin.
2. Blender found that media through current locator evidence, adopted the
   existing asset, and attached its own qualified strip identifier. The
   production contained one logical asset, not one per host.
3. Blender recorded a completed still render as a derived representation with
   an input snapshot and observed render facts. It did not claim a synthetic
   job. The artifact was current, while the recorded knowledge did not pretend
   to be a complete reproducible recipe.
4. Kdenlive read the Blender-origin revision and derived representation without
   opening the `.blend` file.
5. The OpenAssetIO Manager resolved the render's PostProject entity reference
   to its `LocatableContentTrait` location from the same production.
6. Kdenlive confirmed a moved source location; Blender then found and relinked
   the source through PostProject without opening the `.kdenlive` file.
7. Blender and Kdenlive proposed different locator confirmations from one base
   revision. Blender committed first. Kdenlive received a structured
   `locator_set` conflict naming the base and superseding revision, and its
   transaction committed nothing.
8. Kdenlive observed replacement source bytes. Its proxy and Blender's render
   both evaluated stale from their recorded dependencies.

The executable driver uses each maintained host path and public PostProject
surfaces. It does not edit SQLite directly, scan for another host's outputs, or
read another host's project format. The scenario is part of the Kdenlive pilot
CI, and the three integrations retain their independent suites.

The [2026-10-01 CI run](https://github.com/postproject-org/postproject-kdenlive/actions/runs/36847303316)
passed both full Kdenlive builds and the shared-production scenario at pilot
revision `b0102cf`, against PostProject `934b0fe`. The Arch container uses the
installed `uv` executable to create an isolated Python 3.13 Manager environment:
the system Python is externally managed, and OpenAssetIO 1.0.2 has no CPython
3.14 wheel. The scenario receives the same interpreter used to install its
dependencies.

## What was awkward

- Kdenlive's explicit shared-production selector is currently a launch option;
  Blender has an add-on preference and file browser. The path is explicit in
  both cases, but the user experience is not symmetrical.
- Kdenlive observes Blender's revision and render in adapter coverage; the
  pilot has no production-results panel or persistent revision-notification UI.
- Both adapters receive structured conflicts, but neither pilot currently has
  an interactive conflict dialog. The automated loser validates the conflict
  key and revision fields without parsing the diagnostic message.
- Blender exposes render completion and cancellation handlers but no reliable
  render-failure terminal. Recording completed output after the fact is more
  accurate than presenting the extension as a PostProject worker.
- Representation ordering is not semantic. An early scenario driver selected
  the first representation and could target the proxy instead of the original;
  selecting explicitly by representation kind fixed the driver.

No finding required a distributed store, service, automatic merge, new domain
concept, or host-project parser. Explicit production selection, known-media
lookup, revision origin, base revisions, structured conflicts, and existing
provenance/staleness operations represented the demonstrated workflow.

## Verification limitation

The automated background scenario is verified. An interactive pass through
both graphical applications has not been performed in this environment, so no
claim is made about interactive selection clarity or conflict presentation.
