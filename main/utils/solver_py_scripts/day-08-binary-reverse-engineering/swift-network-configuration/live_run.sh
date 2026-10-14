#!/bin/sh
# Swift Justice (day 08). One-shot timed run: start/retry -> download -> solve -> submit. Deadline can be ~7 s, so everything must be in ONE process.
# usage: live_run.sh [start|retry] [mode: time_trial|mastery]   (needs HUNTRESS_GM_TOKEN)
OP=${1:-start}; MODE=${2:-time_trial}
CH=binary-reverse-engineering-day-08-swift-network-configuration/$MODE
H=https://2026.huntress.ctf.games
HERE=$(cd "$(dirname "$0")" && pwd)
W=$HERE/artifacts/$MODE/run_$(date +%s); mkdir -p "$W"
post() { curl -s -X POST $H/ -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -H 'Content-Type: application/json' -d "$1"; }
post "{\"op\":\"$OP\",\"challenge\":\"$CH\"${3:+,\"run_for_time\":true}}" > "$W/start.json"
curl -s "$H$(jq -r .download "$W/start.json")" -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -o "$W/evidence.zip"
mkdir "$W/ev" && 7z x -y -pinfected -o"$W/ev" "$W/evidence.zip" >/dev/null
python3 -I "$HERE/solve.py" "$W/ev" > "$W/answers.json"
jq -n --arg c "$CH" --slurpfile a "$W/answers.json" '{op:"submit",challenge:$c,answers:$a[0]}' > "$W/submit.json"
post "$(cat "$W/submit.json")" > "$W/result.json"
jq -c 'del(.questions,.guidance,.prompt)' "$W/result.json"; jq -r '.deadline' "$W/start.json"
