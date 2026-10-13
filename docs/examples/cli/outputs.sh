#!/usr/bin/env bash
# Record a completed operation; the host supplies the already-produced bytes.
set -euo pipefail
cd "$1"
printf 'fixture render' > render.dat
printf 'fixture thumbnail' > thumbnail.dat
postproject init outputs.pproj
# [register-outputs]
cat > parameters.json <<'JSON'
[{"vocabulary":"example:recipe","property":"width","value":{"type":"i64","value":1920}}]
JSON
postproject --json artifact register outputs.pproj render.dat thumbnail.dat \
  --activity-kind example:generate --tool-name "Fixture generator" \
  --tool-version 1.0 --parameters parameters.json > registered.json
# Add --asset ID --kind derived --input REPRESENTATION_ID for a recorded input.
# Without --asset, each output explicitly creates a new original asset.
jq '{activity_id, outputs, commit_receipt}' registered.json
# [/register-outputs]
test "$(jq '.outputs | length' registered.json)" = 2
test "$(postproject --json activity list outputs.pproj | jq length)" = 1
