#!/bin/sh
# usage: submit_subset.sh answers.json id [id...]   (needs HUNTRESS_GM_TOKEN)
f=$1; shift
jq -n --slurpfile a "$f" --args '{op:"submit",challenge:"binary-reverse-engineering-day-04-go-runtime-fingerprints",answers:($a[0]|with_entries(select(.key as $k|$ARGS.positional|index($k))))}' "$@" \
 | curl -s -X POST https://2026.huntress.ctf.games/ -H "Authorization: Bearer $HUNTRESS_GM_TOKEN" -H 'Content-Type: application/json' -d @- | jq -c 'del(.questions,.guidance,.prompt)'
