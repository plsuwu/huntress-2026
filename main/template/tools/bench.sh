#!/usr/bin/env bash
# Offline before/after benchmark of two solver binaries (needs hyperfine).
#   tools/bench.sh OLD NEW [lane...]      lanes default to every registered lane
# Per lane: the full `replay <fixtures>` suite (throughput, compiles amortised), then each fixture of the newest day
# in a fresh process (closest to a live run: one process, cold caches). Both binaries must pass the replay first.
# Profiling: build with frame pointers + symbols (Rust: RUSTFLAGS="-C force-frame-pointers=yes"
# CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=true), then `perf record -g`. On hybrid CPUs compare
# `perf stat -e cpu_core/cycles/u` too: wall time alone is noisy there.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$(pwd)
cfg() { python3 "$ROOT/tools/orchcfg.py" "$@"; }
OLD=$(realpath "$1"); NEW=$(realpath "$2"); shift 2
cd "$(cfg get solver.dir)"
FIX=$(cfg get solver.fixtures_dir)
if [ $# -gt 0 ]; then LANES=("$@"); else mapfile -t LANES < <(cfg lanes fixtures); fi
OUT=$(mktemp -d)
for l in "${LANES[@]}"; do
  for b in "$OLD" "$NEW"; do "$b" replay "$FIX/$l" 2>&1 | tail -1 | grep -q " clean" || { echo "$b: replay $FIX/$l not clean" >&2; exit 1; }; done
  hyperfine -N -w 1 -r 5 --export-json "$OUT/s.json" -n old "$OLD replay $FIX/$l" -n new "$NEW replay $FIX/$l" >/dev/null 2>&1
  jq -r --arg k "$l (suite)" '[.results[] | .median*1000] | "\($k): \(.[0]|floor) -> \(.[1]|floor) ms (\(((.[1]-.[0])/.[0]*100)|floor)%)"' "$OUT/s.json"
  day=$(ls "$FIX/$l" | grep -oE '^d[0-9]+' | sort -u | tail -1)
  for f in "$FIX/$l/$day"-*.json; do
    [[ -f $f ]] || continue
    hyperfine -N -w 2 -r 15 --export-json "$OUT/f.json" -n old "$OLD replay $f" -n new "$NEW replay $f" >/dev/null 2>&1
    jq -r --arg k "  $(basename "$f" .json)" '[.results[] | .median*1000] | "\($k): \(.[0]*10|floor/10) -> \(.[1]*10|floor/10) ms"' "$OUT/f.json"
  done
done
rm -r "$OUT"
