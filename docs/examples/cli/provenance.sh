#!/usr/bin/env bash
# Runs the CLI listings of the provenance and dependency guide.
#
# Each "[name]" ... "[/name]" region is included verbatim by the documentation
# build, so keep regions self-contained and readable. Usage:
#   PATH=/opt/postproject/bin:$PATH provenance.sh WORK_DIRECTORY
# The work directory is prepared by prepare-workdir.cmake. Requires jq.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: provenance.sh WORK_DIRECTORY" >&2
  exit 2
fi
cd "$1"

postproject init provenance.pproj --name "Documentary"
postproject --json media add provenance.pproj rushes/A001.mov > import.json
ASSET_ID=$(jq -r .asset_id import.json)
ORIGINAL_ID=$(jq -r .representation_id import.json)
RESOURCE_ID=$(jq -r .resource_id import.json)

mkdir -p proxies thumbnails comp
printf 'proxy media' > proxies/A001_proxy.mov
printf 'thumbnail' > thumbnails/A001.jpg
printf 'comp script v1' > comp/shot010.nk

add_file_representation() {
  printf '{"structure": "single_file", "path": "%s"}\n' "$3" > "$2.json"
  postproject --json representation add provenance.pproj "$ASSET_ID" "$1" \
    "$2.json" | jq -r .representation_id
}
PROXY_ID=$(add_file_representation proxy proxy proxies/A001_proxy.mov)
THUMBNAIL_ID=$(add_file_representation derived thumbnail thumbnails/A001.jpg)
COMP_ID=$(add_file_representation derived comp comp/shot010.nk)
# A thumbnail made from the proxy gives the original a second descendant.
postproject activity add provenance.pproj org.example:thumbnail \
  --input "$PROXY_ID" --output "$THUMBNAIL_ID" --tool-name "Example Thumbnailer"

# [activity-snapshots]
postproject activity add provenance.pproj org.postproject:transcode \
  --input "$ORIGINAL_ID=org.postproject:source" --output "$PROXY_ID" \
  --tool-name ffmpeg --tool-version 7.1 --tool-uri https://ffmpeg.org \
  --agent-name "Proxy farm" \
  --agent-identifier-scheme com.example.host --agent-identifier-value render-07
# Each edge keeps a snapshot of the fingerprints current when it was recorded.
postproject --json activity list provenance.pproj |
  jq '.[] | {kind, tool, agent,
    inputs: [.inputs[] | {representation_id, role, fingerprints: .snapshot.fingerprints}],
    outputs: [.outputs[] | {representation_id, fingerprints: .snapshot.fingerprints}]}'
postproject activity consuming provenance.pproj "$ORIGINAL_ID"
# Descendant queries are paged; follow next_cursor until it is null.
CURSOR=
while :; do
  ARGS=(--json activity descendants provenance.pproj "$ORIGINAL_ID" --limit 1)
  [[ -n "$CURSOR" ]] && ARGS+=(--cursor "$CURSOR")
  PAGE=$(postproject "${ARGS[@]}")
  jq -r '.items[] | "\(.representation_id) depth \(.depth)"' <<<"$PAGE"
  CURSOR=$(jq -r '.next_cursor // empty' <<<"$PAGE")
  [[ -z "$CURSOR" ]] && break
done
# [/activity-snapshots]

ACTIVITIES=$(postproject --json activity list provenance.pproj)
test "$(jq length <<<"$ACTIVITIES")" = 2
TRANSCODE=$(jq '.[] | select(.kind == "org.postproject:transcode")' <<<"$ACTIVITIES")
test "$(jq -r '.tool.name + " " + .tool.version' <<<"$TRANSCODE")" = "ffmpeg 7.1"
test "$(jq -r '.agent.identifier.value' <<<"$TRANSCODE")" = render-07
test "$(jq -r '.inputs[0].representation_id' <<<"$TRANSCODE")" = "$ORIGINAL_ID"
test "$(jq -r '.inputs[0].snapshot.fingerprints[0].value_hex' <<<"$TRANSCODE")" = \
  "$(postproject --json representation list provenance.pproj "$ASSET_ID" |
    jq -r --arg id "$ORIGINAL_ID" '.items[] | select(.id == $id) | .fingerprints[0].value_hex')"
test "$(postproject --json activity consuming provenance.pproj "$ORIGINAL_ID" |
  jq -r '[.items[].kind] | join(" ")')" = org.postproject:transcode
test "$(postproject --json activity descendants provenance.pproj "$ORIGINAL_ID" |
  jq -c '[.items[] | [.representation_id, .depth]] | sort_by(.[1])')" = \
  "[[\"$PROXY_ID\",1],[\"$THUMBNAIL_ID\",2]]"

# [stale-after-change]
printf 'regraded camera original' > rushes/A001.mov
postproject media fingerprint provenance.pproj "$RESOURCE_ID" rushes/A001.mov
# Evaluation compares the recorded input snapshots with current fingerprints.
postproject --json artifact evaluate provenance.pproj "$PROXY_ID" |
  jq -r '.state, (.reasons[] | "\(.kind) \(.edge) \(.representation_id)")'
postproject --json artifact reproducibility provenance.pproj "$PROXY_ID" |
  jq -r '"reproducible: \(.reproducible)", (.issues[] | .kind)'
# [/stale-after-change]

EVALUATION=$(postproject --json artifact evaluate provenance.pproj "$PROXY_ID")
test "$(jq -r .state <<<"$EVALUATION")" = stale
test "$(jq -r '.reasons[0].kind + " " + .reasons[0].representation_id' <<<"$EVALUATION")" = \
  "fingerprint_changed $ORIGINAL_ID"
# The transcode recorded no parameters, so it cannot be reproduced exactly.
test "$(postproject --json artifact reproducibility provenance.pproj "$PROXY_ID" |
  jq -r '"\(.reproducible) \([.issues[].kind] | join(","))"')" = "false parameters_missing"

# [dependency-set]
# The recorded array replaces the representation's complete dependency set.
cat > dependencies.json <<EOF
[
  {
    "kind": "org.example:plate",
    "target": {"kind": "asset", "id": "$ASSET_ID"},
    "resolved_representation_id": "$ORIGINAL_ID",
    "authored_reference": "rushes/A001.mov"
  },
  {
    "kind": "org.example:preview",
    "target": {"kind": "representation", "id": "$PROXY_ID"},
    "required": false,
    "authored_reference": "proxies/A001_proxy.mov"
  }
]
EOF
BASE=$(postproject --json inspect provenance.pproj | jq -r .decision_base)
postproject --decision-base "$BASE" dependency record provenance.pproj "$COMP_ID" dependencies.json
postproject dependency show provenance.pproj "$COMP_ID"
CURSOR=
while :; do
  ARGS=(--json dependency dependencies provenance.pproj "$COMP_ID" --limit 1)
  [[ -n "$CURSOR" ]] && ARGS+=(--cursor "$CURSOR")
  PAGE=$(postproject "${ARGS[@]}")
  jq -r '.items[] | "\(.target.kind) \(.target.id) depth \(.depth)"' <<<"$PAGE"
  CURSOR=$(jq -r '.next_cursor // empty' <<<"$PAGE")
  [[ -z "$CURSOR" ]] && break
done
# Once the comp itself changes, its recorded dependencies need re-extraction.
printf 'comp script v2' > comp/shot010.nk
COMP_RESOURCE_ID=$(postproject --json representation resources provenance.pproj \
  "$COMP_ID" | jq -r '.items[0].id')
postproject media fingerprint provenance.pproj "$COMP_RESOURCE_ID" \
  comp/shot010.nk
postproject --json dependency show provenance.pproj "$COMP_ID" | jq -r .status
# [/dependency-set]

DEPENDENCIES=$(postproject --json dependency show provenance.pproj "$COMP_ID")
test "$(jq -r .status <<<"$DEPENDENCIES")" = needs_extraction
test "$(jq -r '[.dependencies[].kind] | join(" ")' <<<"$DEPENDENCIES")" = \
  "org.example:plate org.example:preview"
test "$(jq -r '.dependencies[1] | "\(.required) \(.target.id)"' \
  <<<"$DEPENDENCIES")" = "false $PROXY_ID"
test "$(postproject --json dependency dependencies provenance.pproj "$COMP_ID" \
  --limit 1 | jq -r '.next_cursor != null')" = true
test "$(postproject --json dependency dependencies provenance.pproj "$COMP_ID" |
  jq '.items | length')" = 2
