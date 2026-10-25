# Release-night runbook

Days release at **23:00 AEST** (Australia/Brisbane, no DST). Day N is that date's day of the month. All paths are
relative to the repo root unless they start with `ctf-run/`.

## 0. Before 22:45

- Commit everything to `master`: lane worktrees branch from the committed HEAD.
- Check that the jumpbox answers (`HuntressCTF@10.1.181.90`, key `huntress-id_ed25519`). It needs the VPN: if both
  `10.200.0.1` and the jumpbox stop answering pings, the VPN is down. If ssh times out while `10.200.0.1` still pings,
  the jumpbox has been powered off: it gets shut down automatically after long uptime. Ask the user to bring it up.
  The lanes don't need it, but ranked runs do.

- Check free disk: `df -h /`. Agent scratchpads (`/tmp/nix-shell.*`), five worktree `target/` dirs and run dirs all
  live on the root disk. On Day 24 it filled up mid-rush (ENOSPC), which crashed the driver, broke merges and blinded
  every shell. Want 20 GB+ free before starting.

## 1. Start rush (manually: the systemd timer was removed on 2026-10-20)

```sh
tmux new -d -s rush-driver "nix develop $PWD --command python3 -u ctf-run/tools/rush/rush.py all"   # from the repo root
```

Start it at about 22:45. `all` runs four stages:

1. **prep:** resets the lane worktrees `../wt/<lane>` to `master` on branch `dayNN/<lane>`, symlinks `ctf-run/fixtures`
   and `.env`, and pre-builds every target. About 2 min.
2. **launch:** opens tmux `-L huntress`, session `rush`, with one primed Claude per lane.
3. **watch:** polls for the release every 1 s from 22:59:50.
4. **first pass:** each lane runs single-solve, then TT + mastery once single is CORRECT (binary: mastery after TT).
   Each lane gets a brief in `ctf-run/runs-dNN/rush/brief-<lane>.md`, and its agent is told to read it.

The old unit ran rush with `RuntimeMaxSec=3h` and `KillMode=process`, so the tmux server outlived it. The full design is
in the `rush.py` docstring. Other subcommands:

| Command | What it does |
|---|---|
| `rush.py status --day N` | First-pass results, done markers, stalled agents |
| `rush.py takeover --day N --lanes x` | Finish a stalled lane (untimed only) |
| `rush.py watch --day N --rehearse [--lanes x] [--no-agents]` | Rehearse on a released day |

Attach with `tmux -L huntress attach -t rush`.

Launcher details that matter:

- Agents get `--add-dir` for `ctf-run/` and `utils/` only. Not the repo root: the jumpbox key lives there.
- Agents get `--allowedTools "Bash(./target/release/ctf-run *)"`. On Day 16 the auto-mode classifier blocked plain
  untimed starts without it. That rule does not match `cd <wt> && ./target/release/ctf-run ...`.
- Agents are pinned to `--model claude-opus-5-5 --effort high`. Worktrees have no settings file, so without the pin
  they would inherit the user's per-model effort.
- Everything automated runs with `CTF_NO_RANKED=hof`, so ctf-run refuses only `--timed`. Any other value, e.g. `=1`,
  restores the old full guard: no `--timed`, no `--one-shot-ok`, no starts on timed-by-default routes.
- Each agent's settings file `runs-dNN/rush/settings-<lane>.json` has a `Stop` hook, which writes `stopped-<lane>`,
  and a heartbeat, which writes `active-<lane>`. After the briefs, if an agent stopped after its brief and shows no
  activity for 180 s with no done marker, rush writes `stalled-<lane>.md` (the pane tail).
- The `ctf-run/fixtures` symlink in each worktree is excluded via `.git/info/exclude`.
- **Release detection:** `warmups-day-NN` status going 404 -> 200, with `op=list` as a fallback. A released day's
  global list takes 4-10 s, so each lane then fetches its own routes with `op=list&day=N&search=<prefix>NN`
  (~1.5-3.5 s) and merges them into the route cache under a lock. The `lane=` filter rejects binary-RE.
- **Takeover** chains modes the same way the first pass does, but it holds back a timed-by-default or one-shot route
  that already has an attempt.
- **Relaunch:** `all` kills the previous day's tmux session before relaunching, **but not with `--rehearse`**: a late
  start then logs "tmux session already exists - not relaunching" and keeps yesterday's agents. Before a late start, run
  `tmux -L huntress kill-session -t rush` (and check the old `claude` pids are gone). Never `pkill -f "rush.py ..."`
  from the coordinator shell: the pattern matches the shell itself and kills it.

## 2. Watch the lanes

- **New lanes:** check `op=list&day=N` for lanes outside the five known ones, and do not engage with them (see
  `policies.md`).
- **Markers:** watch `ctf-run/runs-dNN/rush/` for `done-*` and `stalled-*`. For a stalled lane, either message its
  agent or run `rush.py takeover`, then review, commit on the lane branch and `touch done-<lane>`.
- **Messaging an agent by hand:** press Escape first. An open picker ("Teach auto mode...") eats typed messages, which
  is why rush's `send()` presses Escape first. But don't press Escape while the agent runs a long oracle.
- **Hung shell calls:** the agents' `grep` is ugrep, which recurses from cwd when `$(find ...)` expands to nothing.
  Kill that process.
- **Check the GM directly:** `curl '…?op=status&challenge=<id>[/mode]'` on all 15 routes gives `result`,
  `ranked_attempts_remaining` and `hall_of_fame_eligible`. Before the first ranked run of a route,
  `hall_of_fame_eligible` reads `false`, which is normal.

## 3. Merge and deploy

```sh
cd ctf-run && tools/rush/merge.sh N            # merges every lane with a done marker; cargo test + full replay
cd ctf-run && tools/rush/merge.sh N --deploy   # ... and ships the static binary to the jumpbox (tools/deploy.sh)
```

- **Merging early:** finished lanes can merge while others are still working. The replay will then report a few
  failing cases: these are the unfinished lanes' new day-N fixtures, shared through the `fixtures` symlink. This is
  expected; the merges are already committed. Deploy only once the replay is fully clean.
- **Timing:** deploy only between campaigns. `deploy.sh` removes the jumpbox binary before the scp, which would break
  a running campaign.
- **Dirty tree:** `nix build` warns "Git tree is dirty" whenever there are uncommitted files. Untracked run dirs are
  harmless.

## 4. Ranked campaign (only with approval; see `policies.md`)

**Check jumpbox disk first** (`df -h ~` on the box): every ranked run saves its deliveries under `runs-hof/`, and a
binary-RE run keeps up to ~32 MB per evidence zip. On 2026-10-25 a full pass filled the 38 GB disk after ~95 binary
runs and the campaign died on ENOSPC. Want 8 GB+ free; otherwise prune with `ctf-run/tools/prune_hof_runs.sh` (keeps
the newest run's zips per route and mode, and all JSON). Build plans with `ctf-run/tools/hof_plan.py`.

```sh
H=HuntressCTF@10.1.181.90; K=huntress-id_ed25519
ssh -i $K $H 'cd ~/ctf-run-v2 && ./target/release/ctf-run routes --refresh day-NN'   # jumpbox route cache
ctf-run/target/release/ctf-run routes --refresh day-NN                              # local cache, for the survey
HUNTRESS_GM_TOKEN=... python3 ctf-run/tools/hof_survey.py --days NN-NN --json ctf-run/runs-hof/survey-dNN.json
# plan: [{"route": "<base>/<mode>", "rft": true, "budget": 3}, ...] - budget 2 on a 16-attempt pool (binary mastery)
scp -i $K ctf-run/runs-hof/plan-dNN.json "${H}:ctf-run-v2/runs-hof/"
timeout 15 ssh -i $K $H 'cd ~/ctf-run-v2 && setsid nohup python3 -u tools/hof_campaign.py runs-hof/plan-dNN.json \
  runs-hof/campaign-dNN.jsonl > runs-hof/campaign-dNN.log 2>&1 < /dev/null &'
```

- **Survey arguments:** `--days` needs a range, so write `20-20`, not `20`.
- **Launching:** the launch ssh often hangs, so wrap it in `timeout`. The campaign keeps running regardless.
- **Is it still running?** Use `ps -eo args | grep "^[p]ython3 -u tools/hof_campaign"`. A bare `pgrep -f` matches the
  remote shell itself, and `pkill -f` on a pattern that appears in the ssh command line kills your own session.
- **Stopping rules:** the campaign stops a route on #1, on budget, after 3 runs without improvement, or on any
  non-CORRECT result. Look before spending more.
- **Old-day failures:** if a fixture-clean lane starts failing ranked runs on old days, stop that lane's campaign.
  See `grading-lessons.md` for the re-grade procedure.

## Coordinator shell gotchas (zsh)

- `for v in $L` does not word-split: use `${=L}`.
- `scp f $H:path` parses `$H:c` as a history modifier: write `"${H}:path"`.
- Unmatched globs abort the command (`no matches found`), including `grep --include=*.py`. Use `git grep`, or wrap
  the command in `bash -c`.
- `ps | grep -c` exits 1 on zero matches. Don't treat that as an ssh failure.
- On Day 20 the auto-mode classifier refused an `scp` of campaign logs from the jumpbox into the repo. Ask the user
  to approve or copy them; the logs stay in `~/ctf-run-v2/runs-hof/` on the jumpbox either way.
- When the jumpbox is slow to accept connections (2026-10-22: ~39 s per ssh handshake), open one multiplexed master
  and route everything through it: `ssh -i $K -o ConnectTimeout=150 -o ServerAliveInterval=15 -M -S /tmp/claude-1000/jb/s -fN $H`,
  then `ssh -S /tmp/claude-1000/jb/s $H ...` and `scp -o ControlPath=/tmp/claude-1000/jb/s ...` take under 1 s. Keep the socket
  path short: Unix sockets cap it at 108 bytes, and the session scratchpad path is too long. `deploy.sh` opens its own
  connections, so do its four steps by hand through the socket if it times out.
- **No tzdata in the coordinator shell:** `TZ=Australia/Brisbane date` silently falls back to UTC, and so does
  `TZ=Australia/Sydney`. On Day 25 this made a "sleep until 23:00 AEST" timer target 23:00 UTC. The machine's local
  zone is already AEST, so use plain `date` (e.g. `date -d "today 23:00" +%s`).
- **`ls glob | wc -l` under `shopt -s nullglob`:** with no matches, the glob vanishes and `ls` lists the cwd instead,
  so a "5 done markers" check fired at once on Day 25. Count with an array: `m=($d/done-*); ${#m[@]}`.
