#!/bin/sh
# One-process mastery run: start|retry|practice -> download deliveries -> mastery.py -> submit.
# NOTE day 10: run_for_time_supported=false, so a plain `start` is already the timed attempt (28 s); sending run_for_time errors.
# usage: mastery_run.sh [start|practice|retry] [timed]   (needs HUNTRESS_GM_TOKEN). Without `timed` it is a regular untimed attempt.
OP=${1:-start}
CH=binary-reverse-engineering-day-10-zig-cross-platform-services/mastery
H=https://2026.huntress.ctf.games
HERE=$(cd "$(dirname "$0")" && pwd)
W=$HERE/artifacts/mastery/mastery_$(date +%s); mkdir -p "$W"
post() { curl -s -X POST $H/ -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -H 'Content-Type: application/json' -d "$1"; }
post "{\"op\":\"$OP\",\"challenge\":\"$CH\"${2:+,\"run_for_time\":true}}" > "$W/start.json"
jq -c 'del(.questions,.guidance,.prompt,.answer_ids)' "$W/start.json"
n=0
for u in $(jq -r '.downloads[]? // .download' "$W/start.json"); do
  n=$((n+1)); curl -s "$H$u" -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -o "$W/delivery$n.zip" &   # parallel: 28 s deadline
done
wait
python3 -I "$HERE/mastery.py" "$W"
jq -n --arg c "$CH" --slurpfile a "$W/answers.json" '{op:"submit",challenge:$c,answers:$a[0]}' > "$W/submit_body.json"
post "$(cat "$W/submit_body.json")" > "$W/result.json"
jq -c 'del(.questions,.guidance,.prompt)' "$W/result.json"
