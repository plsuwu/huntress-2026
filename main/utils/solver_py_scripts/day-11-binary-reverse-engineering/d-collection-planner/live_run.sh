#!/bin/sh
# D-Day (day 11). One-shot run: start/retry -> download -> solve -> submit. Deadline ~10 s, so everything is in ONE process.
# usage: live_run.sh [start|retry] [time_trial|mastery]   (needs HUNTRESS_GM_TOKEN; route is timed by default (run_for_time_supported=false): do NOT send run_for_time, it returns timed_intent_not_available)
OP=${1:-start}; MODE=${2:-time_trial}
CH=binary-reverse-engineering-day-11-d-collection-planner/$MODE
H=https://2026.huntress.ctf.games
HERE=$(cd "$(dirname "$0")" && pwd)
W=$HERE/artifacts/$MODE/run_$(date +%s); mkdir -p "$W"
post() { curl -s -X POST $H/ -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -H 'Content-Type: application/json' -d "$1"; }
post "{\"op\":\"$OP\",\"challenge\":\"$CH\"}" > "$W/start.json"
curl -s "$H$(jq -r .download "$W/start.json")" -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -o "$W/evidence.zip"
mkdir "$W/ev" && 7z x -y -pinfected -o"$W/ev" "$W/evidence.zip" >/dev/null
SOLVER=${SOLVER:-solve.py}
python3 -I "$HERE/$SOLVER" "$W/ev" > "$W/answers.json"
jq -n --arg c "$CH" --slurpfile a "$W/answers.json" '{op:"submit",challenge:$c,answers:$a[0]}' > "$W/submit.json"
post "$(cat "$W/submit.json")" > "$W/result.json"
jq -c 'del(.questions,.guidance,.prompt)' "$W/result.json"; jq -c '{deadline,started_at,unique_attempts_remaining,hall_of_fame_eligible}' "$W/start.json"
