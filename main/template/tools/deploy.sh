#!/bin/sh
# Ship the solver to the run host (config: [run_host]). Builds the static binary locally - never compile on a small
# or burstable run host before ranked runs - then: copies the tools, uploads the binary under a temporary name, keeps
# the previous one as <bin>.prev-<hash>, swaps atomically, smoke-tests, and refreshes nothing else.
#   tools/deploy.sh             build + ship
#   tools/deploy.sh --no-build  ship the existing static result
# Uses one multiplexed ssh connection (run_host.control_path): slow handshakes are paid once. Don't deploy while a
# campaign is running on the host - the swap is atomic, but a campaign mid-run would mix binaries.
set -eu
cd "$(dirname "$0")/.."                                   # repo root
cfg() { python3 tools/orchcfg.py get "$@"; }
HOST=$(cfg run_host.host); KEY=$(cfg run_host.key); RDIR=$(cfg run_host.dir); SOCK=$(cfg run_host.control_path)
BIN=$(cfg solver.bin); RESULT=$(cfg run_host.static_result)
SSH="ssh -S $SOCK -o ControlMaster=auto -o ControlPersist=10m -o BatchMode=yes -o ConnectTimeout=150 -o ServerAliveInterval=15 -i $KEY"
if [ "${1:-}" != "--no-build" ]; then $(cfg run_host.static_build); fi
[ -x "$RESULT" ] || { echo "no static build at $RESULT" >&2; exit 1; }
NEW=$(sha256sum "$RESULT" | cut -c1-8)
echo "deploying $NEW to $HOST:$RDIR/$BIN"
$SSH "$HOST" "ps -eo args | grep -q '^[p]ython3 -u tools/campaign' && { echo 'a campaign is running - not deploying' >&2; exit 1; } || true"
tar czf - tools | $SSH "$HOST" "mkdir -p $RDIR && cd $RDIR && rm -rf tools && tar xzf -"
$SSH "$HOST" "mkdir -p $RDIR/$(dirname "$BIN")"
scp -q -o ControlPath="$SOCK" -i "$KEY" "$RESULT" "$HOST:$RDIR/$BIN.tmp"
$SSH "$HOST" "cd $RDIR && chmod 755 $BIN.tmp && if [ -e $BIN ]; then cp -p $BIN $BIN.prev-\$(sha256sum $BIN | cut -c1-8); fi && mv $BIN.tmp $BIN && sha256sum $BIN | cut -c1-8 && ./$BIN lanes >/dev/null && echo deployed"
echo "rollback: $SSH $HOST 'cd $RDIR && mv <bin>.prev-<hash> $BIN'"
echo "next: refresh the host's route cache, then an untimed live check on the day's routes before any ranked run"
