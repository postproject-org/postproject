#!/usr/bin/env bash
# Installed development CLI: authority, checkpoint, identified recovery and suffix.
set -euo pipefail
cd "$1"
postproject --json init authority.pproj --name Documentary > production.json

# [exchange-checkpoint]
postproject exchange position authority.pproj cursor.json
postproject exchange checkpoint export authority.pproj knowledge.ppxc
postproject exchange checkpoint validate knowledge.ppxc
postproject exchange checkpoint import knowledge.ppxc mirror.pproj
# [/exchange-checkpoint]

# [exchange-submit]
CLIENT=00000000-0000-4000-8000-000000000001
REQUEST=00000000-0000-4000-8000-000000000002
jq --arg client "$CLIENT" --arg request "$REQUEST" '{
  kind: "proposal", version: "1", required_features: ["metadata.v1"],
  production, history, client: $client, request: $request,
  base: null, origin: null, message: null, commands: [], extensions: {}
}' cursor.json > proposal.json
postproject --json exchange submit authority.pproj proposal.json > accepted.json
postproject --json exchange outcome authority.pproj \
  --history "$(jq -r .history cursor.json)" \
  --client "$CLIENT" --request "$REQUEST" > recovered.json
test "$(jq -c .outcome accepted.json)" = "$(jq -c .outcome recovered.json)"
test "$(jq -r .outcome.receipt.revision accepted.json)" = null
# [/exchange-submit]

# [exchange-catch-up]
# Ordinary native writes automatically enter the complete source history.
postproject --json media add authority.pproj rushes/A001.mov > asset.json
postproject exchange changes export authority.pproj changes.ppxd --from cursor.json
postproject --json exchange changes apply mirror.pproj changes.ppxd > applied.json
postproject --json exchange inspect mirror.pproj > mirror.json
test "$(jq -c .head applied.json)" = "$(jq -c .head mirror.json)"
test "$(jq -r .role mirror.json)" = passive_mirror
# [/exchange-catch-up]

postproject --json exchange changes apply mirror.pproj changes.ppxd > duplicate.json
cmp applied.json duplicate.json
postproject --json media list mirror.pproj > assets.json
jq -e --arg id "$(jq -r .asset_id asset.json)" 'any(.[]; .id == $id)' assets.json
if postproject media add mirror.pproj rushes/A001.mov 2> rejected.txt; then
  echo "passive mirror unexpectedly accepted a native write" >&2
  exit 1
fi
