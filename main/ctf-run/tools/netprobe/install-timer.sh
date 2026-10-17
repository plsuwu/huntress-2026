#!/bin/sh
# Installs (or with --remove, removes) the systemd user timer that runs collect.py every 10 minutes (at :05, :15, ...): one small
# read-only burst from the jumpbox against the Game Master proxy, appended to ctf-run/runs/netprobe/probes.jsonl
# (gitignored). Summarise with report.py. Follow it: journalctl --user -fu huntress-netprobe
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
UNIT_DIR="$HOME/.config/systemd/user"
if [ "${1:-}" = "--remove" ]; then
  systemctl --user disable --now huntress-netprobe.timer 2>/dev/null || true
  rm -f "$UNIT_DIR/huntress-netprobe.service" "$UNIT_DIR/huntress-netprobe.timer"
  systemctl --user daemon-reload
  echo "removed"; exit 0
fi
mkdir -p "$UNIT_DIR"
cat > "$UNIT_DIR/huntress-netprobe.service" <<UNIT
[Unit]
Description=Huntress CTF network probe: one latency burst from the jumpbox to the Game Master proxy

[Service]
Type=oneshot
WorkingDirectory=$REPO
Environment=PATH=/etc/profiles/per-user/$USER/bin:/run/current-system/sw/bin
ExecStart=/etc/profiles/per-user/$USER/bin/python3 -u $HERE/collect.py
TimeoutStartSec=150
UNIT
cat > "$UNIT_DIR/huntress-netprobe.timer" <<UNIT
[Unit]
Description=Huntress CTF network probe (every 10 minutes)

[Timer]
# :05 offset keeps probes off the 23:00:00 release second. Plain minute spec, no named zone (systemd here misreads named time zones - see rush/install-timer.sh).
OnCalendar=*:05/10
AccuracySec=5s
Persistent=false
Unit=huntress-netprobe.service

[Install]
WantedBy=timers.target
UNIT
systemctl --user daemon-reload
systemctl --user enable --now huntress-netprobe.timer
systemctl --user list-timers huntress-netprobe.timer --no-pager
