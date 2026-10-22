# Release-night runbook (fill in for this event)

The step-by-step version of `../playbooks/orchestrator.md` §4, with this event's commands. Keep it current.

## Before the release
- Commit everything to the main branch.
- Run host reachable? (`ssh ... uptime`; if slow, open a multiplexed master: see deploy.sh)
- Route caches fresh (local and run host).

## Rush
```sh
tmux new -d -s rush-driver "python3 -u tools/rush/rush.py all"            # ~15 min before the release
tmux new -d -s rush-driver "python3 -u tools/rush/rush.py all --day N --rehearse"   # an already-released day
python3 tools/rush/rush.py status --day N
python3 tools/rush/rush.py takeover --day N --lanes <lane>
```
Attach: `tmux -L <agents.tmux_socket> attach -t <agents.session>` (`-r` = read-only).

## Merge and deploy
```sh
tools/merge.sh N            # merges lanes with done markers; build + tests + full replay
tools/deploy.sh             # static build, ship, keep previous binary, atomic swap
```
Then: refresh the host's route cache, untimed live check on the day's routes.

## Campaign (only with approval)
```sh
python3 tools/survey.py --days N-N --json docs/survey-dN.json --plan plan-dN.json
# edit the plan, copy it to the run host, then launch detached there:
setsid nohup python3 -u tools/campaign.py plan-dN.json campaign-dN.jsonl > campaign-dN.log 2>&1 < /dev/null &
```
Is it running? `ps -eo args | grep "^[p]ython3 -u tools/campaign"` (a bare `pgrep -f` matches your own ssh shell).

## Shell gotchas
- zsh: `for v in $L` does not word-split (`${=L}`); unmatched globs abort; `scp f $H:path` needs `"${H}:path"`.
- `ps | grep -c` exits 1 on zero matches.
