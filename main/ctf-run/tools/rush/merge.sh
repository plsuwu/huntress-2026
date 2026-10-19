#!/bin/sh
# Coordinator: merge every lane branch whose agent touched its done-marker, then gate the merged tree.
#   tools/rush/merge.sh DAY [--deploy]
# Merges day<DAY>/<lane> into master (stops at the first conflict - resolve, commit, rerun), then runs cargo test and
# the full fixture replay. --deploy also builds the static binary and ships it to the jumpbox (tools/deploy.sh) for
# ranked runs. Lanes without a marker are listed and left alone, so fast lanes can ship before slow ones.
set -eu
DAY=$(printf %02d "$1")
cd "$(dirname "$0")/../.."                       # ctf-run/
STATE="runs-d$DAY/rush"
[ "$(git rev-parse --abbrev-ref HEAD)" = master ] || { echo "main checkout is not on master" >&2; exit 1; }
merged=""
for lane in warmups metadata script binary mobile web; do
  branch="day$DAY/$lane"
  if [ ! -e "$STATE/done-$lane" ]; then echo "$lane: not done yet"; continue; fi
  if ! git rev-parse -q --verify "$branch" >/dev/null; then echo "$lane: no branch $branch"; continue; fi
  if [ -z "$(git log --oneline "master..$branch")" ]; then echo "$lane: nothing to merge"; continue; fi
  echo "$lane: merging $branch"
  git merge --no-ff --no-edit "$branch" || { echo "CONFLICT merging $branch - resolve, commit, rerun" >&2; exit 1; }
  merged="$merged $lane"
done
[ -n "$merged" ] || { echo "nothing merged"; exit 0; }
cargo build --release --features tls 2>&1 | grep -E '^(error|warning: unused)' || true
cargo test --release 2>&1 | grep -E 'test result|FAILED|panicked'
fail=0
for f in warmups mobile metadata binary_re script; do
  out=$(./target/release/ctf-run replay "fixtures/$f" 2>&1 | tail -1)
  echo "replay $f: $out"
  case "$out" in *"case(s) clean"*) n=$(echo "$out" | grep -oE '[0-9]+/[0-9]+'); [ "${n%/*}" = "${n#*/}" ] || fail=1 ;; *) fail=1 ;; esac
done
[ $fail = 0 ] || { echo "REPLAY NOT CLEAN - do not deploy" >&2; exit 1; }
echo "merged:$merged - tests and replay clean"
if [ "${2:-}" = "--deploy" ]; then tools/deploy.sh; fi
