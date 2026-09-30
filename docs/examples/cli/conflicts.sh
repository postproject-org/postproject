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
ROOT_ID=$(postproject --json root add conflicts.pproj rushes | jq -r .id)
BASE_ID=$(postproject --json revisions latest conflicts.pproj | jq -r .id)

postproject --base-revision "$BASE_ID" root disable conflicts.pproj "$ROOT_ID"

if postproject --json --base-revision "$BASE_ID" \
  root enable conflicts.pproj "$ROOT_ID" 2>conflict.json; then
  echo "stale write unexpectedly succeeded" >&2
  exit 1
fi
jq -e --arg root "$ROOT_ID" --arg base "$BASE_ID" \
  '.error.transaction_conflict
   | .key.kind == "media_root"
     and .key.target_id == $root
     and .base_revision_id == $base' conflict.json >/dev/null

# Retry only after re-reading and deciding that enabling is still right.
REFRESHED_ID=$(postproject --json revisions latest conflicts.pproj | jq -r .id)
postproject --base-revision "$REFRESHED_ID" \
  root enable conflicts.pproj "$ROOT_ID"
# [/semantic-conflicts]
