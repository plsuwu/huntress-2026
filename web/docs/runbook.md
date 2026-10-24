# Web lane release-night runbook

Days release at **23:00 AEST** (Australia/Brisbane, no DST). Day N is that date's day of the
month. The pipeline is `tools/web_rush.py` — the web adaptation of the main pipeline's
`rush.py`. All paths are relative to this repo root.

## 0. Before 22:45

- Commit everything to `master`. Workers commit their own day directories
  (`solver_py_scripts/day-NN-web-exploitation/<slug>/`, with the usual Co-Authored-By
  line); the coordinator commits shared-doc updates. Disjoint paths — no branches, no
  merge step.
- `HUNTRESS_GM_TOKEN` is in `.env` (or exported). Never print it; it is never sent to a
  target.
- Sanity: `python3 -m py_compile tools/web_rush.py tools/wk.py`, then
  `python3 tools/web_rush.py status --day N-1` — yesterday's challenges should read
  `done`, or the day was never run.
- The tmux socket should be free: `tmux -L huntress-web ls` says "no server running"
  (`all` kills the previous session itself, but a leftover from an aborted run is worth
  a look). Never use, attach to, signal or kill the sibling repo's `-L huntress` socket.

## 1. Start the driver

```sh
tmux new -d -s web-driver "python3 -u tools/web_rush.py all --day N"   # from the repo root
```

Start it at about 22:45. `all` runs three stages:

1. **prep:** worker dirs `workers/web/<base-id>/` for every challenge in the route cache
   (usually none yet), the shared trees symlinked in (`AGENTS.md`, `docs/`,
   `solver_py_scripts/`, `tools/`), stale `.rush-dNN` markers cleared.
2. **launch:** tmux `-L huntress-web`, session `web` — a `control` window plus one primed
   Claude window per pre-warmed slot (`--workers N`, default 3). Each slot's agent reads
   `tools/prompts/prime.md`, replies "ready" and waits for the release.
3. **watch:** polls `op=list&day=N&lane=web-exploitation` every 1 s from 22:59:50 and
   releases on two consecutive identical polls. At the release: one first-pass thread per
   challenge (pure automation over `solver_py_scripts/recovery_lib.py`, untimed only),
   a brief per challenge at `workers/web/.rush-dNN/brief-<chal>.md`, dispatched to its
   agent as soon as that challenge's pass ends. Slot i gets challenge i; a day with more
   challenges than slots gets extra windows on the spot, fewer stands the spares down.
   Then the stall monitor runs.

Watch the driver's log: `tmux attach -t web-driver`. Watch the agents:
`tmux -L huntress-web attach -t web` (detach with Ctrl-b d).

Other subcommands:

| Command | What it does |
|---|---|
| `web_rush.py prep --day N` | Worker dirs + symlinks from the route cache, stale markers cleared |
| `web_rush.py launch --day N [--workers N]` | The primed tmux session (refuses to run if it already exists) |
| `web_rush.py watch --day N [--rehearse] [--no-agents]` | Release wait + first pass + briefs (+ dispatch) |
| `web_rush.py status --day N` | First-pass results, done markers, stalled agents |
| `web_rush.py takeover --day N --challenges <short>[,...]` | Fresh worker window + brief for a stalled challenge |

`--rehearse` skips the release wait and dispatches no agents: already-solved routes are
recognised from GM status with zero starts, which makes it a safe preflight on a released
day. `--no-agents` runs the first pass and writes briefs without any tmux dispatch.

## 2. Launcher details that matter

- Workers get `--add-dir` for `docs/`, `solver_py_scripts/` and `tools/` only — never the
  repo root: the jumpbox SSH key and `.env` live there, and workers never get them as cwd.
- Workers get `--allowedTools "Bash(python3 <root>/tools/wk.py *)"` so the auto-mode
  classifier cannot stall a submit (the Day 16 lesson). Everything else runs under
  `--permission-mode auto`.
- Workers are pinned to `--model claude-opus-5-5 --effort high` (their cwd carries no
  settings file to inherit from).
- The GM token reaches a pane only through `tmux -e` (the session environment), and the
  worker uses it only through `tools/wk.py`. The driver redacts it from its own error
  messages.
- Each pane's settings file has a `Stop` hook (writes `stopped-<chal>`) and a heartbeat
  (`PostToolUse`/`UserPromptSubmit` write `active-<chal>`). Marker names resolve through
  `slot-<slot>.chal`, so the prime turn's stops never start a stall clock. After the
  briefs, an agent that stopped after its brief, shows no activity for 180 s and has no
  `done-<chal>` marker is escalated to `stalled-<chal>.md` (pane tail + takeover command).
- **Untimed only, everywhere:** the driver's first pass and `wk.py` send only
  `list`/`status`/`start`/`submit` — never `run_for_time`, `retry`, `practice`, `reset` or
  `switch_target`. Plain starts on dual-pool routes are free untimed regular attempts.
- **Capacity:** a `409 web_target_limit_reached` on start waits (bounded) for a target
  slot; it never disturbs another family's target. An untimed target stops after 30 idle
  minutes, so the hand-off to the worker happens while the first pass's targets are warm.
- **Takeover:** `web_rush.py takeover` writes a brief (pane tail + GM status per route +
  worker-dir listing), kills the stalled window and launches a fresh worker whose prompt
  is the brief. `wk.py begin` is idempotent, so the replacement reconnects to the same
  attempt — no work is lost.
- **Relaunch:** `all` kills the previous day's `web` session before relaunching.

## 3. Watch the challenges

- `python3 tools/web_rush.py status --day N` — one line per challenge: first-pass steps
  plus agent state (`done` / `STOPPED N s ago` / `working` / `not dispatched`).
- Markers live in `workers/web/.rush-dNN/`: `brief-<chal>.md`, `firstpass-<chal>.json`,
  `done-<chal>`, `stalled-<chal>.md`, `takeover-<chal>.md`.
- **Messaging an agent by hand:** press Escape first — an open picker ("Teach auto
  mode…") eats typed messages, which is why the driver's `send()` presses Escape first.
  But don't press Escape while the agent runs a long oracle.
- **Stalled agent:** either message it
  (`tmux -L huntress-web send-keys -t web:<chal> ...`) or run
  `python3 tools/web_rush.py takeover --day N --challenges <chal>`.
- **The worker's loop** (what each pane is doing):
  `python3 tools/wk.py begin <mode>` → `submit <mode>` → `status` / `next`,
  single_solve → time_trial → mastery, serial within the family.
- **Check the GM directly (read-only):**
  `curl 'https://2026.huntress.ctf.games/?op=status&challenge=<route>' -H "Authorization: Bearer $HUNTRESS_GM_TOKEN"`.
- Evidence lives in `workers/web/<base-id>/<mode>/` (`start.json`, `targets.json`,
  `provisioning.json`, recon dumps, `answers.json`, `results.json`, `NOTES.md`,
  `solve.py`). All of `workers/web/` is git-ignored; keep it for future reference.
- **Shared scratch tmpfs can fill under concurrent slots.** Day 24 saw every Bash
  call (even `echo`) fail with ENOSPC on one worker's scratch tmpfs partway
  through a run with multiple slots active, then recover on its own after a
  pause with no code change. Non-Bash tools (Read/Write to project paths) kept
  working throughout. This is sandbox/infra contention, not a challenge or
  solver bug — if it recurs, wait and retry rather than debugging the solve.py.

## 4. After the night: merge the notes

Workers commit their own day directories. The coordinator then (~10 min):

1. Fold new bug-class methods into `docs/web-worker-lessons.md`.
2. Fold the day's outcome + next-day prediction into `docs/lane-patterns.md`.
3. Fold any new `rec_*` recoverer into `solver_py_scripts/recovery_lib.py`'s
   `classify_and_recover` dispatch — after reviewing the graded evidence. This is what
   makes the next day's first pass stronger.
4. Append a dated entry to `docs/history.md`.
5. Commit the shared-doc updates (one commit per day is fine).

## 5. Standing rules (details in docs/policies.md)

- Web-exploitation routes only; never list, query, start, download or submit anything else.
- Untimed only; no timed, ranked, retry, practice, reset or switch_target — there is no
  override in this pipeline. Hall of Fame campaigns for web stay a coordinator decision
  outside this pipeline.
- The token is never printed, never sent to a target, and workers use it only through
  `tools/wk.py`.
- Never use, attach to, signal or kill the sibling repo's `-L huntress` socket. Kill the
  `web` session only when no driver is running.
- If the GM starts answering 429/5xx at scale, say so: the no-gating decision (2026-10-24)
  gets revisited and a shared lock comes back.
