#!/usr/bin/env bash
# Offline before/after benchmark of two ctf-run binaries (needs hyperfine; in the repo devShell).
#   tools/bench.sh OLD NEW [lane...]   e.g. tools/bench.sh /tmp/ctf-run.master target/release/ctf-run
# Per lane: the full `replay fixtures/<lane>` suite (CPU throughput, amortised compiles), then each Day-NN
# time-trial/mastery fixture in a fresh process (closest to a live run: one process, cold caches).
# Both binaries must pass the replay first. For profiling: CTF_PROFILE=1, or perf with
# RUSTFLAGS="-C force-frame-pointers=yes" CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=true.
set -euo pipefail
cd "$(dirname "$0")/.."
OLD=$(realpath "$1"); NEW=$(realpath "$2"); shift 2
LANES=("${@:-warmups mobile metadata script binary_re}")
OUT=$(mktemp -d)
for l in ${LANES[@]}; do
  for b in "$OLD" "$NEW"; do "$b" replay "fixtures/$l" 2>&1 | tail -1 | grep -q " clean" || { echo "$b: replay fixtures/$l not clean" >&2; exit 1; }; done
  r=10; [[ $l == binary_re || $l == script ]] && r=5
  hyperfine -N -w 1 -r $r --export-json "$OUT/s.json" -n old "$OLD replay fixtures/$l" -n new "$NEW replay fixtures/$l" >/dev/null 2>&1
  jq -r --arg k "$l (suite)" '[.results[] | .median*1000] | "\($k): \(.[0]|floor) -> \(.[1]|floor) ms (\(((.[1]-.[0])/.[0]*100)|floor)%)"' "$OUT/s.json"
  day=$(ls fixtures/$l | grep -oE '^d[0-9]+' | sort -u | tail -1)
  for f in fixtures/$l/$day-time_trial*.json fixtures/$l/$day-mastery*.json; do
    [[ -f $f ]] || continue
    r=20; [[ $l == binary_re ]] && r=8
    hyperfine -N -w 2 -r $r --export-json "$OUT/f.json" -n old "$OLD replay $f" -n new "$NEW replay $f" >/dev/null 2>&1
    jq -r --arg k "  $(basename "$f" .json)" '[.results[] | .median*1000] | "\($k): \(.[0]*10|floor/10) -> \(.[1]*10|floor/10) ms"' "$OUT/f.json"
  done
done
rm -r "$OUT"
