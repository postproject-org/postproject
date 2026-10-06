#!/usr/bin/env bash
set -euo pipefail
cd "$1"

# [root-pages]
postproject init roots.pproj
postproject root add roots.pproj first --priority=-1
postproject root add roots.pproj second
postproject --json root page roots.pproj --limit 1 >first.json
jq -e '.items | length == 1' first.json >/dev/null
CURSOR=$(jq -r .next_cursor first.json)
postproject --json root page roots.pproj --limit 1 --cursor "$CURSOR" >last.json
jq -e '.items[0].name == "second" and .next_cursor == null' last.json >/dev/null
# CLI continuations resume a live query; they do not retain a previous view.
# [/root-pages]
