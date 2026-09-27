#!/usr/bin/env bash
# Runs the CLI listings of the identifier and metadata guide.
#
# Each "[name]" ... "[/name]" region is included verbatim by the documentation
# build, so keep regions self-contained and readable. Usage:
#   PATH=/opt/postproject/bin:$PATH knowledge.sh WORK_DIRECTORY
# The work directory is prepared by prepare-workdir.cmake. Requires jq.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: knowledge.sh WORK_DIRECTORY" >&2
  exit 2
fi
cd "$1"

postproject init knowledge.pproj --name "Documentary"
postproject --json media add knowledge.pproj rushes/A001.mov > import.json
ASSET_ID=$(jq -r .asset_id import.json)
ORIGINAL_ID=$(jq -r .representation_id import.json)

# [remove-identifier]
postproject identifier add knowledge.pproj asset "$ASSET_ID" \
  com.example.camera.serial A-0007
postproject identifier add knowledge.pproj asset "$ASSET_ID" \
  com.example.tape B-0112 --qualifier reel
postproject identifier list knowledge.pproj asset "$ASSET_ID"
postproject identifier find knowledge.pproj com.example.camera.serial A-0007
# A qualifier restricts the lookup to identifiers with exactly that qualifier.
postproject identifier find knowledge.pproj com.example.tape B-0112 \
  --qualifier reel
# Removal matches the exact scheme, value, and qualifier.
postproject identifier remove knowledge.pproj asset "$ASSET_ID" \
  com.example.tape B-0112 --qualifier reel
# [/remove-identifier]

test "$(postproject --json identifier list knowledge.pproj asset "$ASSET_ID" |
  jq -r '[.[].scheme] | join(" ")')" = com.example.camera.serial
test "$(postproject --json identifier find knowledge.pproj \
  com.example.camera.serial A-0007 | jq -r '.[0].id')" = "$ASSET_ID"
test "$(postproject --json identifier find knowledge.pproj \
  com.example.tape B-0112 | jq length)" = 0

# A second object carrying "take" gives the paged query below two pages.
echo '{"type": "u64", "value": 1}' > representation-take.json
postproject metadata add knowledge.pproj representation "$ORIGINAL_ID" \
  https://example.com/ns/editorial/1 take representation-take.json

# [typed-metadata]
EDITORIAL=https://example.com/ns/editorial/1
mkdir -p values
# One JSON value file per property; "type" selects the metadata value type.
cat > values/slug.json <<'EOF'
{"type": "string", "value": "INT-001"}
EOF
cat > values/title.json <<'EOF'
{"type": "lang_string", "value": "Interview", "language": "en-US"}
EOF
cat > values/offset.json <<'EOF'
{"type": "i64", "value": -12}
EOF
cat > values/take.json <<'EOF'
{"type": "u64", "value": 3}
EOF
cat > values/gain.json <<'EOF'
{"type": "decimal", "coefficient": "-35", "scale": 1}
EOF
cat > values/approved.json <<'EOF'
{"type": "bool", "value": true}
EOF
cat > values/shot-at.json <<'EOF'
{"type": "timestamp", "unix_micros": 1767225600000000}
EOF
cat > values/homepage.json <<'EOF'
{"type": "uri", "value": "https://example.com/interview"}
EOF
cat > values/checksum.json <<'EOF'
{"type": "bytes", "hex": "00ff10"}
EOF
cat > values/rate.json <<'EOF'
{"type": "rational", "numerator": 24000, "denominator": 1001}
EOF
cat > values/keywords.json <<'EOF'
{"type": "list", "values": [
  {"type": "string", "value": "interview"},
  {"type": "string", "value": "exterior"}
]}
EOF
cat > values/camera.json <<'EOF'
{"type": "struct", "fields": [
  {"name": "model", "value": {"type": "string", "value": "A-Cam"}},
  {"name": "iso", "value": {"type": "u64", "value": 800}}
]}
EOF
cat > values/source.json <<EOF
{"type": "reference",
 "target": {"target_kind": "representation", "target_id": "$ORIGINAL_ID"}}
EOF
for VALUE_FILE in values/*.json; do
  postproject metadata add knowledge.pproj asset "$ASSET_ID" "$EDITORIAL" \
    "$(basename "$VALUE_FILE" .json)" "$VALUE_FILE"
done
postproject --json metadata list knowledge.pproj asset "$ASSET_ID" |
  jq -r '.[] | .property + ": " + (.value | if .type == "lang_string" then
      "\(.value) (\(.language))"
    elif .type == "decimal" then "\(.coefficient)e-\(.scale)"
    elif .type == "timestamp" then "\(.unix_micros) us"
    elif .type == "bytes" then "0x\(.hex)"
    elif .type == "rational" then "\(.numerator)/\(.denominator)"
    elif .type == "list" then [.values[].value] | join(", ")
    elif .type == "struct" then [.fields[] | "\(.name)=\(.value.value)"] | join(", ")
    elif .type == "reference" then "\(.target.kind) \(.target.id)"
    else .value | tostring end)'
# Metadata queries are paged; follow next_cursor until it is null.
CURSOR=
while :; do
  ARGS=(--json metadata find knowledge.pproj "$EDITORIAL" take --limit 1)
  [[ -n "$CURSOR" ]] && ARGS+=(--cursor "$CURSOR")
  PAGE=$(postproject "${ARGS[@]}")
  jq -r '.items[] | "\(.target_kind) \(.target_id) take \(.value.value)"' <<<"$PAGE"
  CURSOR=$(jq -r '.next_cursor // empty' <<<"$PAGE")
  [[ -z "$CURSOR" ]] && break
done
# [/typed-metadata]

LISTED=$(postproject --json metadata list knowledge.pproj asset "$ASSET_ID")
test "$(jq length <<<"$LISTED")" = 13
test "$(jq -r '[.[].value.type] | sort | join(" ")' <<<"$LISTED")" = \
  "bool bytes decimal i64 lang_string list rational reference string struct timestamp u64 uri"
test "$(jq -c '.[] | select(.property == "gain") | .value' <<<"$LISTED")" = \
  '{"type":"decimal","coefficient":"-35","scale":1}'
test "$(jq -r '.[] | select(.property == "source") | .value.target.id' <<<"$LISTED")" = \
  "$ORIGINAL_ID"

FIRST_PAGE=$(postproject --json metadata find knowledge.pproj "$EDITORIAL" take --limit 1)
test "$(jq '.items | length' <<<"$FIRST_PAGE")" = 1
SECOND_PAGE=$(postproject --json metadata find knowledge.pproj "$EDITORIAL" take \
  --limit 1 --cursor "$(jq -r .next_cursor <<<"$FIRST_PAGE")")
test "$(jq '.items | length' <<<"$SECOND_PAGE")" = 1
test "$(jq -r .next_cursor <<<"$SECOND_PAGE")" = null

postproject metadata add knowledge.pproj asset "$ASSET_ID" "$EDITORIAL" \
  keywords values/keywords.json

# [remove-metadata]
# Removes every value of the property from the object, here both keyword lists.
postproject metadata remove knowledge.pproj asset "$ASSET_ID" "$EDITORIAL" keywords
postproject --json metadata find knowledge.pproj "$EDITORIAL" keywords |
  jq '.items | length'
# [/remove-metadata]

test "$(postproject --json metadata find knowledge.pproj "$EDITORIAL" keywords |
  jq '.items | length')" = 0
test "$(postproject --json metadata list knowledge.pproj asset "$ASSET_ID" |
  jq length)" = 12
