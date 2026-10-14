#!/bin/sh
# One-process mastery run: start|practice -> download deliveries -> mastery.py -> submit (7 s deadline on a timed start).
# usage: mastery_run.sh [start|practice|retry]   (needs HUNTRESS_GM_TOKEN)
OP=${1:-start}
CH=binary-reverse-engineering-day-05-rust-runtime-shapes/mastery
H=https://2026.huntress.ctf.games
HERE=$(cd "$(dirname "$0")" && pwd)
W=$HERE/artifacts/mastery/mastery_$(date +%s); mkdir -p "$W"
post() { curl -s -X POST $H/ -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -H 'Content-Type: application/json' -d "$1"; }
post "{\"op\":\"$OP\",\"challenge\":\"$CH\"}" > "$W/start.json"
n=0
for u in $(jq -r '.downloads[]' "$W/start.json"); do
  n=$((n+1)); curl -s "$H$u" -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -o "$W/delivery$n.zip"
done
python3 -I "$HERE/mastery.py" "$W" >/dev/null
jq -n --arg c "$CH" --slurpfile a "$W/answers.json" '{op:"submit",challenge:$c,answers:$a[0]}' > "$W/submit_body.json"
post "$(cat "$W/submit_body.json")" > "$W/result.json"
jq -c 'del(.questions,.guidance,.prompt)' "$W/result.json"; jq -r '.deadline' "$W/start.json"
