#!/usr/bin/env bash
# Runs the CLI listings of the durable jobs guide.
#
# Each "[name]" ... "[/name]" region is included verbatim by the documentation
# build, so keep regions self-contained and readable. Usage:
#   PATH=/opt/postproject/bin:$PATH jobs.sh WORK_DIRECTORY
# The work directory is prepared by prepare-workdir.cmake. Requires jq.
#
# Job timestamps are always supplied by the caller; the fixed values below
# stand in for a worker's clock.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: jobs.sh WORK_DIRECTORY" >&2
  exit 2
fi
cd "$1"

postproject init jobs.pproj --name "Documentary"
postproject --json media add jobs.pproj rushes/A001.mov > import.json
ASSET_ID=$(jq -r .asset_id import.json)
ORIGINAL_ID=$(jq -r .representation_id import.json)
mkdir -p proxies
postproject root add jobs.pproj proxies --label "Generated proxies"

job_state() {
  postproject --json job list jobs.pproj | jq -r --arg id "$1" \
    '.items[] | select(.id == $id) | .state'
}

# [request-job]
# --profile persists the named proxy profile as typed job metadata.
JOB_ID=$(postproject --json job request jobs.pproj org.postproject:generate-proxy \
  "$ASSET_ID" proxy --input "$ORIGINAL_ID" --target-root proxies \
  --profile proxy-720p | jq -r .id)
postproject --json job list jobs.pproj |
  jq --arg id "$JOB_ID" '.items[] | select(.id == $id) |
    {kind, state, inputs, output_asset_id, output_kind, target_root}'
# [/request-job]

REQUESTED=$(postproject --json job list jobs.pproj --state requested)
test "$(jq -r '.items[0] | "\(.id) \(.kind) \(.output_kind) \(.target_root)"' \
  <<<"$REQUESTED")" = "$JOB_ID org.postproject:generate-proxy proxy proxies"
test "$(jq -r '.items[0].inputs | join(" ")' <<<"$REQUESTED")" = "$ORIGINAL_ID"

# [claim-job]
CLAIM=$(postproject --json job claim jobs.pproj "$JOB_ID" \
  --tool-name "Proxy Worker" --tool-version 1.0 --agent-name worker-1 \
  --now-unix-micros 1767225600000000 --expires-at-unix-micros 1767225900000000)
# The claim ID is the token for every later renew, release, complete, or fail.
CLAIM_ID=$(jq -r .claim_id <<<"$CLAIM")
postproject job renew jobs.pproj "$JOB_ID" "$CLAIM_ID" \
  --now-unix-micros 1767225800000000 --expires-at-unix-micros 1767226100000000
postproject job release jobs.pproj "$JOB_ID" "$CLAIM_ID"
# [/claim-job]

test "$(jq -r '.claim_tool.name + " " + .claim_agent.name' <<<"$CLAIM")" = \
  "Proxy Worker worker-1"
test "$(job_state "$JOB_ID")" = requested
test "$(postproject --json job list jobs.pproj | jq -r '.items[0].claim_id')" = null

# [complete-job]
CLAIM_ID=$(postproject --json job claim jobs.pproj "$JOB_ID" \
  --tool-name "Proxy Worker" --tool-version 1.0 \
  --now-unix-micros 1767226200000000 --expires-at-unix-micros 1767226500000000 |
  jq -r .claim_id)
printf 'proxy media' > proxies/A001_proxy.mov
# Adds the output representation and its producing activity, and marks the
# job succeeded, in one transaction.
COMPLETED=$(postproject --json job complete jobs.pproj "$JOB_ID" "$CLAIM_ID" \
  proxies/A001_proxy.mov --now-unix-micros 1767226300000000)
jq '{state, completion_representation_id, completion_activity_id, commit_receipt}' <<<"$COMPLETED"
# [/complete-job]

jq -e '.commit_receipt.revision.id | type == "string"' <<<"$COMPLETED" >/dev/null
PROXY_ID=$(jq -r .completion_representation_id <<<"$COMPLETED")
test "$(jq -r .state <<<"$COMPLETED")" = succeeded
test "$(postproject --json activity producing jobs.pproj "$PROXY_ID" |
  jq -r '.items[0] | "\(.id) \(.tool.name)"')" = \
  "$(jq -r .completion_activity_id <<<"$COMPLETED") Proxy Worker"

FAILING_ID=$(postproject --json job request jobs.pproj \
  org.postproject:generate-proxy "$ASSET_ID" proxy --input "$ORIGINAL_ID" |
  jq -r .id)
REPRESENTATIONS_BEFORE=$(postproject --json representation list jobs.pproj "$ASSET_ID" |
  jq '.items | length')

# [fail-job]
CLAIM_ID=$(postproject --json job claim jobs.pproj "$FAILING_ID" \
  --tool-name "Proxy Worker" \
  --now-unix-micros 1767226400000000 --expires-at-unix-micros 1767226700000000 |
  jq -r .claim_id)
postproject --json job fail jobs.pproj "$FAILING_ID" "$CLAIM_ID" \
  "ffmpeg exited with status 1" --now-unix-micros 1767226450000000 |
  jq '{state, failure_diagnostic}'
# [/fail-job]

test "$(job_state "$FAILING_ID")" = failed
test "$(postproject --json representation list jobs.pproj "$ASSET_ID" |
  jq '.items | length')" = "$REPRESENTATIONS_BEFORE"

CANCELLED_ID=$(postproject --json job request jobs.pproj \
  org.postproject:generate-proxy "$ASSET_ID" proxy --input "$ORIGINAL_ID" |
  jq -r .id)

# [cancel-job]
postproject job cancel jobs.pproj "$CANCELLED_ID"
# [/cancel-job]

test "$(job_state "$CANCELLED_ID")" = cancelled

# [plan-regeneration]
# Planning only reads recorded knowledge; nothing is enqueued until the
# planned request is submitted explicitly.
postproject --json job plan jobs.pproj --artifact "$PROXY_ID" > plan.json
jq '.[] | {artifact_representation_id, job: (.job | {kind, inputs, output_kind}),
  parameters}' plan.json
PLANNED=$(jq -c '.[0].job' plan.json)
INPUTS=()
for INPUT in $(jq -r '.inputs[]' <<<"$PLANNED"); do INPUTS+=(--input "$INPUT"); done
REGENERATION_ID=$(postproject --json job request jobs.pproj \
  "$(jq -r .kind <<<"$PLANNED")" "$(jq -r .output_asset_id <<<"$PLANNED")" \
  "$(jq -r .output_kind <<<"$PLANNED")" "${INPUTS[@]}" | jq -r .id)
# [/plan-regeneration]

test "$(jq length plan.json)" = 1
test "$(jq -r '.[0].artifact_representation_id' plan.json)" = "$PROXY_ID"
test "$(jq -r '.[0].job.state' plan.json)" = requested
test "$(job_state "$REGENERATION_ID")" = requested
test "$(postproject --json job list jobs.pproj --state requested |
  jq -r '[.items[].id] | join(" ")')" = "$REGENERATION_ID"
