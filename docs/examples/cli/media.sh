#!/usr/bin/env bash
# Runs the CLI listings of the media structure guide.
#
# Each "[name]" ... "[/name]" region is included verbatim by the documentation
# build, so keep regions self-contained and readable. Usage:
#   PATH=/opt/postproject/bin:$PATH media.sh WORK_DIRECTORY
# The work directory is prepared by prepare-workdir.cmake. Requires jq.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: media.sh WORK_DIRECTORY" >&2
  exit 2
fi
cd "$1"

postproject init media.pproj --name "Documentary"
postproject --json media add media.pproj rushes/A001.mov > import.json
jq -e '.commit_receipt.revision.sequence == 1' import.json >/dev/null
ASSET_ID=$(jq -r .asset_id import.json)
ORIGINAL_ID=$(jq -r .representation_id import.json)
RESOURCE_ID=$(jq -r .resource_id import.json)
OLD_LOCATOR_ID=$(jq -r .locator_id import.json)

representation_structure() {
  postproject --json representation list media.pproj "$ASSET_ID" |
    jq -r --arg id "$1" '.items[] | select(.id == $id) | .structure'
}

mkdir -p proxies spanned package
printf 'proxy media' > proxies/A001_proxy.mov
printf 'span one' > spanned/CLIP0001.MTS
printf 'span two' > spanned/CLIP0002.MTS
printf 'essence' > package/clip.mxf
printf '<clip/>' > package/clip.xml

# [add-representation]
cat > proxy.json <<'EOF'
{"structure": "single_file", "path": "proxies/A001_proxy.mov"}
EOF
PROXY_ID=$(postproject --json representation add media.pproj "$ASSET_ID" \
  proxy proxy.json | jq -r .representation_id)
# [/add-representation]

test "$(representation_structure "$PROXY_ID")" = single_resource

# [ordered-parts]
# The parts of one spanned recording form a single representation, in order.
cat > spanned.json <<'EOF'
{
  "structure": "ordered_parts",
  "members": [
    {"path": "spanned/CLIP0001.MTS", "role": "org.postproject:span-part"},
    {"path": "spanned/CLIP0002.MTS", "role": "org.postproject:span-part"}
  ]
}
EOF
SPANNED_ID=$(postproject --json representation add media.pproj "$ASSET_ID" \
  original spanned.json | jq -r .representation_id)
# [/ordered-parts]

test "$(representation_structure "$SPANNED_ID")" = ordered_parts

# [package-representation]
# Members default to required; an optional sidecar may be offline without
# making the package unavailable.
cat > package.json <<'EOF'
{
  "structure": "package",
  "members": [
    {"path": "package/clip.mxf", "role": "org.postproject:essence"},
    {"path": "package/clip.xml", "role": "org.postproject:sidecar",
     "required": false}
  ]
}
EOF
PACKAGE_ID=$(postproject --json representation add media.pproj "$ASSET_ID" \
  optimized package.json | jq -r .representation_id)
# [/package-representation]

test "$(representation_structure "$PACKAGE_ID")" = package
test "$(postproject --json representation resources media.pproj "$PACKAGE_ID" |
  jq '.items | length')" = 2

cat > shot010.json <<'EOF'
{
  "structure": "image_sequence",
  "directory": "renders/shot010",
  "prefix": "shot010.",
  "suffix": ".exr",
  "padding": 4,
  "start": 1001,
  "end": 1004,
  "step": 1,
  "rate_numerator": 24,
  "rate_denominator": 1,
  "missing_frames": [1003]
}
EOF
SEQUENCE_ID=$(postproject --json representation add media.pproj "$ASSET_ID" \
  derived shot010.json | jq -r .representation_id)

# [representation-structure]
# Page through the asset's representations, following each next_cursor.
CURSOR=
while :; do
  ARGS=(--json representation list media.pproj "$ASSET_ID" --limit 2)
  [[ -n "$CURSOR" ]] && ARGS+=(--cursor "$CURSOR")
  PAGE=$(postproject "${ARGS[@]}")
  jq -r '.items[] | "\(.id) \(.kind) \(.structure) \(.fingerprints[0].value_hex)"' \
    <<<"$PAGE"
  CURSOR=$(jq -r '.next_cursor // empty' <<<"$PAGE")
  [[ -z "$CURSOR" ]] && break
done
# Resources in structural order, with their fingerprints and locators.
for RESOURCE in $(postproject --json representation resources media.pproj \
  "$PACKAGE_ID" | jq -r '.items[].id'); do
  postproject --json locator list media.pproj "$RESOURCE" |
    jq -r '.items[] | "\(.resource_id) \(.uri) \(.availability)"'
done
postproject --json media show media.pproj "$ASSET_ID" |
  jq --arg id "$SEQUENCE_ID" '.representations[] | select(.id == $id) |
    .resources[] | {fingerprints, locators: [.locators[].uri]}'
# [/representation-structure]

test "$(postproject --json representation list media.pproj "$ASSET_ID" |
  jq '.items | length')" = 5
test "$(postproject --json representation list media.pproj "$ASSET_ID" --limit 2 |
  jq -r '.next_cursor != null')" = true
test "$(representation_structure "$SEQUENCE_ID")" = image_sequence

# [media-root-lifecycle]
postproject root add media.pproj rushes --label "Camera originals"
ARCHIVE_ID=$(postproject --json root add media.pproj archive --priority 10 |
  jq -r .id)
postproject root list media.pproj
# A disabled root stays configured but is skipped during resolution.
for ACTION in disable enable remove; do
  BASE=$(postproject --json inspect media.pproj | jq -r .decision_base)
  postproject --decision-base "$BASE" root "$ACTION" media.pproj "$ARCHIVE_ID"
done
# [/media-root-lifecycle]

test "$(postproject --json root list media.pproj | jq -r '[.[].name] | join(" ")')" = rushes

mv rushes/A001.mov moved/A001.mov

# [resolve-scope]
# A search directory is an unnamed, machine-local place, such as where the
# media used to be; it is searched after mapped roots and never recorded.
postproject --json media resolve media.pproj "$ASSET_ID" \
  --search-dir "$PWD/moved" |
  jq -r '.resolutions[].resources[] | select(.state == "resolved_exact") |
         .candidates[0] | "\(.uri) (root: \(.media_root // "-"))"'
# [/resolve-scope]

test "$(postproject --json media resolve media.pproj "$ASSET_ID" \
  --search-dir "$PWD/moved" |
  jq -r '[.resolutions[].resources[] | select(.state == "resolved_exact")] |
         length')" = 1

CANDIDATE=$(postproject --json media resolve media.pproj "$ASSET_ID" \
  --root-map rushes="$PWD/moved" |
  jq -r --arg id "$ORIGINAL_ID" \
    '.resolutions[] | select(.representation_id == $id) |
     .resources[0].candidates[0].uri')
postproject media resolve media.pproj "$ASSET_ID" \
  --root-map rushes="$PWD/moved" --confirm "$CANDIDATE"

# [retire-locator]
# The file moved and its new location was confirmed; retire the old route.
BASE=$(postproject --json inspect media.pproj | jq -r .decision_base)
postproject --decision-base "$BASE" locator retire media.pproj "$OLD_LOCATOR_ID"
CURSOR=
while :; do
  ARGS=(--json locator list media.pproj "$RESOURCE_ID" --limit 100)
  [[ -n "$CURSOR" ]] && ARGS+=(--cursor "$CURSOR")
  PAGE=$(postproject "${ARGS[@]}")
  jq -r '.items[] | "\(.uri) (root: \(.media_root // "-"))"' <<<"$PAGE"
  CURSOR=$(jq -r '.next_cursor // empty' <<<"$PAGE")
  [[ -z "$CURSOR" ]] && break
done
# [/retire-locator]

LOCATORS=$(postproject --json locator list media.pproj "$RESOURCE_ID")
test "$(jq '.items | length' <<<"$LOCATORS")" = 1
test "$(jq -r '.items[0].uri' <<<"$LOCATORS")" = "$CANDIDATE"
test "$(jq -r '.items[0].media_root' <<<"$LOCATORS")" = rushes

printf 'unrelated take' > moved/A002.mov

# [inventory-scan]
# Inventory classifies files under mapped roots without changing the
# production; the cache only speeds up repeated scans.
postproject --json media inventory media.pproj --root-map rushes="$PWD/moved" \
  --cache inventory.cache > inventory.json
jq -r '.items[] | "\(.category) \(.uri)"' inventory.json
jq '.stats' inventory.json
# [/inventory-scan]

test "$(jq -r '[.items[] | "\(.category):\(.uri | split("/") | last)"] | sort | join(" ")' \
  inventory.json)" = "known_online:A001.mov new_candidate:A002.mov"

# [fingerprint-observation]
printf 'regraded camera original' > moved/A001.mov
# Content changed, and PostProject's own fingerprint says so.
postproject media verify-content media.pproj "$RESOURCE_ID" moved/A001.mov
# Records the new resource fingerprint, and recomputes every representation
# using the resource, in one revision. The outcome is "changed".
DECISION_BASE=$(postproject --json inspect media.pproj | jq -r .decision_base)
postproject --json --decision-base "$DECISION_BASE" media fingerprint media.pproj "$RESOURCE_ID" moved/A001.mov |
  jq '{outcome, resource_fingerprint, representation_fingerprints}'
REVISION_ID=$(postproject --json revisions latest media.pproj | jq -r .id)
# Observing the same content again reports "unchanged" and records nothing.
DECISION_BASE=$(postproject --json inspect media.pproj | jq -r .decision_base)
postproject --json --decision-base "$DECISION_BASE" media fingerprint media.pproj "$RESOURCE_ID" moved/A001.mov |
  jq -r .outcome
test "$(postproject --json revisions latest media.pproj | jq -r .id)" = "$REVISION_ID"
postproject --json revisions events media.pproj "$REVISION_ID" |
  jq -r '.[] | "\(.position) \(.kind)"'
# [/fingerprint-observation]

test "$(postproject --json revisions events media.pproj "$REVISION_ID" |
  jq -r '[.[].kind] | join(" ")')" = \
  "resource_fingerprint_observed resource_file_facts_observed representation_fingerprint_observed"

# [resolution-issues]
postproject --json media resolve media.pproj "$ASSET_ID" \
  --root-map rushes="$PWD/moved" > resolution.json
jq -r '.resolutions[] | .representation_id as $rep | .issues[] |
  "\($rep) \(.kind) required=\(.required) frames=\(.frames)"' resolution.json
jq -r '.resolutions[].resources[] |
  "\(.resource_id) \(.state) \([.evidence[].kind, .candidates[].evidence[].kind])"' \
  resolution.json
# [/resolution-issues]

test "$(jq -c --arg id "$SEQUENCE_ID" \
  '.resolutions[] | select(.representation_id == $id) | [.availability, .issues[0].kind, .issues[0].frames]' \
  resolution.json)" = '["partial","missing_frames",[1003]]'

# [verify-resolution]
printf 'truncated' > proxies/A001_proxy.mov
# --verify recomputes stored fingerprints for content at known locators, so
# changed bytes are reported instead of trusted.
postproject --json media resolve media.pproj "$ASSET_ID" \
  --root-map rushes="$PWD/moved" --verify |
  jq -r '.resolutions[] | "\(.representation_id) \(.availability) \([.resources[].evidence[].kind])"'
# [/verify-resolution]

test "$(postproject --json media resolve media.pproj "$ASSET_ID" \
  --root-map rushes="$PWD/moved" --verify |
  jq -r --arg id "$PROXY_ID" '.resolutions[] | select(.representation_id == $id) |
    "\(.availability) \(.resources[0].evidence[0].kind)"')" = "error fingerprint_mismatch"

mkdir -p card

# [media-inspection]
# A stand-in for ffprobe that prints a fixed probe result.
cat > ffprobe-fake <<'EOF'
#!/bin/sh
cat <<'JSON'
{"format": {"format_name": "mov,mp4", "duration": "2.5"},
 "streams": [{"index": 0, "codec_type": "video", "codec_name": "prores",
              "width": 1920, "height": 1080, "avg_frame_rate": "24000/1001"}]}
JSON
EOF
chmod +x ffprobe-fake
printf 'camera B' > card/B001.mov
postproject --json media add media.pproj card/B001.mov --name "Camera B" \
  --inspect --ffprobe "$PWD/ffprobe-fake" > inspected.json
jq -r '.inspections[] | "\(.path): \(.status)"' inspected.json
postproject --json metadata list media.pproj representation \
  "$(jq -r .representation_id inspected.json)" |
  jq '.[] | select(.vocabulary == "https://postproject.org/ns/technical-media/1")'
# [/media-inspection]

test "$(jq -r '.inspections[0].status' inspected.json)" = recorded
test "$(postproject --json metadata list media.pproj representation \
  "$(jq -r .representation_id inspected.json)" |
  jq -r '.[0].value.fields[] | select(.name == "container") | .value.value')" = "mov,mp4"

# [media-recognition]
printf 'camera C' > card/C001.mov
printf '<clip id="C001"/>' > card/C001.xml
# A same-stem .xml, .xmp, or .json sidecar joins the file as an optional
# package member instead of becoming a separate asset.
postproject --json media add media.pproj card/C001.mov --recognize-companions \
  > recognized.json
postproject --json representation list media.pproj \
  "$(jq -r .asset_id recognized.json)" | jq -r '.items[] | "\(.kind) \(.structure)"'
# [/media-recognition]

test "$(jq -r .resource_count recognized.json)" = 2
test "$(postproject --json representation list media.pproj \
  "$(jq -r .asset_id recognized.json)" | jq -r '.items[0].structure')" = package

# [import-sequence]
# A directory holding one numbered image group imports as one asset whose
# only original representation is the sequence; frame 1003 is missing.
postproject --json media add media.pproj renders/shot010 --name "shot010 strip" \
  --sequence-rate 24/1 > strip.json
postproject --json representation list media.pproj \
  "$(jq -r .asset_id strip.json)" | jq -r '.items[] | "\(.kind) \(.structure)"'
# [/import-sequence]

test "$(postproject --json representation list media.pproj \
  "$(jq -r .asset_id strip.json)" | jq -r '[.items[] | "\(.kind) \(.structure)"] | join(",")')" = \
  "original image_sequence"

STRIP_ID=$(jq -r .asset_id strip.json)
mkdir -p graded
for FRAME in 1001 1002 1004; do
  mv "renders/shot010/shot010.$FRAME.exr" "graded/shot010-graded_$FRAME.exr"
done

# [relink-renamed-sequence]
# A renamed sequence is found by content; each candidate carries the naming
# its files have now, as a printf-style pattern.
postproject --json media resolve media.pproj "$STRIP_ID" --search-dir "$PWD/graded" |
  jq -r '.resolutions[].resources[].candidates[] |
         "\(.uri) \(.sequence_naming.pattern) \([.evidence[].kind])"'
# Confirmation records the candidate with its naming; --confirm-naming picks
# one when several namings were found in the same directory.
postproject media resolve media.pproj "$STRIP_ID" --search-dir "$PWD/graded" \
  --confirm "$(postproject --json media resolve media.pproj "$STRIP_ID" \
    --search-dir "$PWD/graded" |
    jq -r '.resolutions[0].resources[0].candidates[0].uri')" \
  --confirm-naming 'shot010-graded_%04d.exr'
# [/relink-renamed-sequence]

test "$(postproject --json media show media.pproj "$STRIP_ID" |
  jq -r '[.representations[0].resources[0].locators[].sequence_naming.pattern] |
         sort | join(" ")')" = "shot010-graded_%04d.exr shot010.%04d.exr"
