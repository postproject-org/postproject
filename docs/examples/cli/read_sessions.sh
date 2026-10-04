#!/usr/bin/env bash
set -euo pipefail
cd "$1"

# [coherent-reads]
postproject init views.pproj
BASE=$(postproject --json inspect views.pproj --limit 10 | jq -r .decision_base)
postproject --json --decision-base "$BASE" root add views.pproj rushes >created.json
jq -e '.commit_receipt.revision.sequence == 1' created.json >/dev/null
ROOT=$(jq -r .id created.json)

# The initial empty base must not silently overwrite the newly created root.
if postproject --json --decision-base "$BASE" root disable views.pproj "$ROOT" \
  2>conflict.json; then
  echo "stale edit unexpectedly succeeded" >&2
  exit 1
fi
jq -e '.error.transaction_conflict.base_revision_id == null' conflict.json >/dev/null

# Inspect again, decide again, then pass the refreshed scoped token.
BASE=$(postproject --json inspect views.pproj --limit 10 | jq -r .decision_base)
postproject --json --decision-base "$BASE" root disable views.pproj "$ROOT" >updated.json
jq -e '.commit_receipt.revision.sequence == 2' updated.json >/dev/null
BASE=$(postproject --json inspect views.pproj --limit 10 | jq -r .decision_base)
postproject --json --decision-base "$BASE" root disable views.pproj "$ROOT" >noop.json
jq -e '.commit_receipt.revision == null' noop.json >/dev/null
# [/coherent-reads]
