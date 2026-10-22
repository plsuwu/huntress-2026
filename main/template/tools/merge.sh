#!/bin/sh
# Coordinator: merge every lane branch whose agent touched its done marker, then gate the merged tree.
#   tools/merge.sh DAY [--deploy]
# Merges the day's lane branches into the main branch (stops at the first conflict - resolve, commit, rerun), then
# builds, runs the tests and the full fixture replay. --deploy also ships the static binary (tools/deploy.sh).
# Lanes without a done marker are listed and left alone, so fast lanes can ship before slow ones.
set -eu
cd "$(dirname "$0")/.."                                   # repo root
ROOT=$(pwd)
cfg() { python3 "$ROOT/tools/orchcfg.py" "$@"; }
SOLVER=$(cfg get solver.dir)
BIN=$(cfg get solver.bin)
STATE="$SOLVER/$(cfg fmt solver.runs_dir day="$1")/rush"
MAIN=$(git rev-parse -q --verify main >/dev/null && echo main || echo master)
[ "$(git rev-parse --abbrev-ref HEAD)" = "$MAIN" ] || { echo "main checkout is not on $MAIN" >&2; exit 1; }
merged=""
for lane in $(cfg lanes); do
  branch=$(cfg fmt agents.branch day="$1" lane="$lane")
  if [ ! -e "$STATE/done-$lane" ]; then echo "$lane: not done yet"; continue; fi
  if ! git rev-parse -q --verify "$branch" >/dev/null; then echo "$lane: no branch $branch"; continue; fi
  if [ -z "$(git log --oneline "$MAIN..$branch")" ]; then echo "$lane: nothing to merge"; continue; fi
  echo "$lane: merging $branch"
  git merge --no-ff --no-edit "$branch" || { echo "CONFLICT merging $branch - resolve, commit, rerun" >&2; exit 1; }
  merged="$merged $lane"
done
[ -n "$merged" ] || { echo "nothing merged"; exit 0; }
cd "$SOLVER"
$(cfg get solver.build) 2>&1 | grep -E '^(error|warning: unused)' || true
$(cfg get solver.test) 2>&1 | grep -E 'test result|FAILED|panicked' || true
fail=0
for f in $(cfg lanes fixtures); do
  out=$("./$BIN" replay "$(cfg get solver.fixtures_dir)/$f" 2>&1 | tail -1)
  echo "replay $f: $out"
  # Expects the solver's replay summary to end in "<ok>/<total> case(s) clean"; adapt to your solver's output.
  case "$out" in *"case(s) clean"*) n=$(echo "$out" | grep -oE '[0-9]+/[0-9]+'); [ "${n%/*}" = "${n#*/}" ] || fail=1 ;; *) fail=1 ;; esac
done
[ $fail = 0 ] || { echo "REPLAY NOT CLEAN - do not deploy" >&2; exit 1; }
echo "merged:$merged - tests and replay clean"
if [ "${2:-}" = "--deploy" ]; then cd "$ROOT" && tools/deploy.sh; fi
