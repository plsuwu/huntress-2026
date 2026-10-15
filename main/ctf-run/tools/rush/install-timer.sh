#!/bin/sh
# Installs (or with --remove, removes) the systemd user timer that runs `rush.py all` at 22:45 Australia/Brisbane
# every day of the event, 15 minutes before the 23:00 AEST release:
#   22:45 prep (reset + build the lane worktrees) -> launch (tmux + primed lane agents) -> 22:58:30 poll for the
#   release -> first pass per lane -> dispatch briefs. Follow it: journalctl --user -fu huntress-rush;
#   attach: tmux -L huntress attach -t rush. Needs the user session running (you logged in) at that time.
set -eu
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
UNIT_DIR="$HOME/.config/systemd/user"
if [ "${1:-}" = "--remove" ]; then
  systemctl --user disable --now huntress-rush.timer 2>/dev/null || true
  rm -f "$UNIT_DIR/huntress-rush.service" "$UNIT_DIR/huntress-rush.timer"
  systemctl --user daemon-reload
  echo "removed"; exit 0
fi
mkdir -p "$UNIT_DIR"
cat > "$UNIT_DIR/huntress-rush.service" <<EOF
[Unit]
Description=Huntress CTF day release: lane worktrees, primed lane agents, automated first pass

[Service]
Type=simple
WorkingDirectory=$REPO
Environment=PATH=/etc/profiles/per-user/$USER/bin:/run/current-system/sw/bin
# The flake's devShell provides cargo/python3/tmux/claude; the tmux server it starts outlives this unit.
ExecStart=/run/current-system/sw/bin/nix develop $REPO --command python3 -u ctf-run/tools/rush/rush.py all
KillMode=process
RuntimeMaxSec=3h
EOF
cat > "$UNIT_DIR/huntress-rush.timer" <<EOF
[Unit]
Description=Huntress CTF day release (22:45 AEST, event days only)

[Timer]
# 12:45 UTC = 22:45 AEST (Brisbane has no DST). Named zones mis-resolve here: systemd read "Australia/Brisbane" as UTC.
OnCalendar=2026-10-01..31 12:45:00 UTC
AccuracySec=1s
Persistent=false
Unit=huntress-rush.service

[Install]
WantedBy=timers.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now huntress-rush.timer
systemctl --user list-timers huntress-rush.timer --no-pager
