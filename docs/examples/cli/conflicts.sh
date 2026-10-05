#!/usr/bin/env bash
# Runs the CLI listing in the semantic-conflicts guide.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: conflicts.sh WORK_DIRECTORY" >&2
  exit 2
fi
cd "$1"

# [semantic-conflicts]
postproject init conflicts.pproj
CREATED=$(postproject --json root add conflicts.pproj rushes)
ROOT_ID=$(jq -r .id <<<"$CREATED")
BASE_ID=$(jq -r .commit_receipt.revision.id <<<"$CREATED")
BASE=$(postproject --json inspect conflicts.pproj | jq -r .decision_base)

postproject --decision-base "$BASE" root disable conflicts.pproj "$ROOT_ID"

if postproject --json --decision-base "$BASE" \
  root enable conflicts.pproj "$ROOT_ID" >conflict.json; then
  echo "stale write unexpectedly succeeded" >&2
  exit 1
fi
jq -e --arg root "$ROOT_ID" --arg base "$BASE_ID" \
  '.error.transaction_conflict
   | .key.kind == "media_root"
     and .key.target_id == $root
     and .base_revision_id == $base' conflict.json >/dev/null

# Retry only after re-reading and deciding that enabling is still right.
REFRESHED=$(postproject --json inspect conflicts.pproj | jq -r .decision_base)
postproject --decision-base "$REFRESHED" \
  root enable conflicts.pproj "$ROOT_ID"
# [/semantic-conflicts]
