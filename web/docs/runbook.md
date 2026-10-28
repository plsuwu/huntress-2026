# Web lane release-night runbook

Days release at **23:00 AEST** (Australia/Brisbane, no DST). Day N is that date's day of the
month. The pipeline is `tools/web_rush.py` — the web adaptation of the main pipeline's
`rush.py`. All paths are relative to this repo root.

## 0. Before 22:45

- Commit everything to `master`. Workers commit their own note + script
  (`workers/web/solution-notes/day-NN-slug.md` + `workers/web/past-solutions/day-NN-slug-*.py`,
  with the usual Co-Authored-By line); the coordinator commits shared-doc updates. Disjoint
  paths — no branches, no merge step.
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
2. **launch:** starts the shared `opencode serve` process (if not already up), then tmux
   `-L huntress-web`, session `web` — a `control` window plus one `opencode attach` window
   per pre-warmed slot (`--workers N`, default 3). Each slot's session is primed with
   `tools/prompts/prime.md` (reads the docs, replies "ready") before its window opens, so
   the pane shows that exchange already in the transcript.
3. **watch:** polls `op=list&day=N&lane=web-exploitation` every 1 s from 22:59:50 and
   releases on two consecutive identical polls. At the release: one first-pass thread per
   challenge (pure automation over `solver_py_scripts/recovery_lib.py`, untimed only),
   a brief per challenge at `workers/web/.rush-dNN/brief-<chal>.md`, dispatched to its
   session as soon as that challenge's pass ends. Slot i gets challenge i; a day with more
   challenges than slots gets an extra session + window on the spot, fewer stands the
   spares down (their window is just closed - nothing to tell an idle session). Each
   dispatch is one blocking `opencode run` call covering the whole challenge; there is no
   separate stall monitor - see §3.

Watch the driver's log: `tmux attach -t web-driver`. Watch the agents:
`tmux -L huntress-web attach -t web` (detach with Ctrl-b d).

Other subcommands:

| Command | What it does |
|---|---|
| `web_rush.py prep --day N` | Worker dirs + symlinks from the route cache, stale markers cleared |
| `web_rush.py launch --day N [--workers N]` | The primed tmux session (refuses to run if it already exists) |
| `web_rush.py watch --day N [--rehearse] [--no-agents]` | Release wait + first pass + briefs (+ dispatch) |
| `web_rush.py status --day N` | First-pass results, agent state (done / timed out / working / not dispatched) |
| `web_rush.py takeover --day N --challenges <short>[,...]` | Send one more message to a challenge's existing session |

`--rehearse` skips the release wait and dispatches no agents: already-solved routes are
recognised from GM status with zero starts, which makes it a safe preflight on a released
day. `--no-agents` runs the first pass and writes briefs without any tmux dispatch.

## 2. Launcher details that matter

- One shared `opencode serve` process (started by `launch`/`all`) hosts every session for
  the day. Its own environment carries `HUNTRESS_GM_TOKEN`/`HUNTRESS_GM_URL`/
  `WEB_UNTIMED_ONLY=1` - tool execution happens server-side, so every session's Bash tool
  calls inherit it from there, not from a per-window flag. The driver redacts the token
  from its own error messages regardless.
- Workers get `--dir <worker-dir>` (never the repo root: the jumpbox SSH key and `.env`
  live there). File-tree scoping beyond that is the same symlink trick as before
  (`AGENTS.md`, `docs/`, `solver_py_scripts/`, `tools/` symlinked into each worker dir) -
  no `--add-dir`/`--allowedTools` equivalent is needed.
- No permission flag or config is passed. Confirmed live: plain bash tool calls already run
  with zero friction under the environment's default policy, and
  `--dangerously-skip-permissions` is both unneeded and a blanket bypass the auto-mode
  classifier itself rejects. The guard that matters is still `tools/wk.py`'s own code.
- Workers track opencode's own configured default model/effort unless `--model`/`--effort`
  is passed to `web_rush.py` (their cwd carries no project config to override it).
- **No stall detection.** Each challenge's dispatch is one blocking
  `opencode run --attach <url> --session <id> "<brief>"` call that covers the whole
  challenge (every mode, NOTES.md, the commit) - the call returning *is* "done". There is
  no Stop/PostToolUse hook, no heartbeat, no stalled-`<chal>`.md escalation: confirmed that
  opencode has no hook mechanism to build one from anyway, and the user judged it not worth
  keeping. A challenge marked `running` in `status` just means its thread is still blocked
  on that call - watch its tmux pane if you want to see what it's doing.
- **Untimed only, everywhere:** the driver's first pass and `wk.py` send only
  `list`/`status`/`start`/`submit` plus the guarded worker `reset` — never
  `run_for_time`, `retry`, `practice` or `switch_target`. Plain starts on dual-pool
  routes are free untimed regular attempts.
- **Capacity:** a `409 web_target_limit_reached` on start waits (bounded) for a target
  slot; it never disturbs another family's target. An untimed target stops after 30 idle
  minutes, so the hand-off to the worker happens while the first pass's targets are warm.
- **Takeover:** `web_rush.py takeover` sends one more message to the challenge's existing
  session - nothing is lost, since the tmux window is already attached to that same live
  session (no pane-tail capture, no fresh-window relaunch; `wk.py begin` is idempotent
  regardless). Attaching tmux and typing into the pane directly works just as well - both
  are just clients of the same session, and a human can type into a pane even while the
  driver's own call is still in flight on it.
- **Relaunch:** `all` kills the previous day's `web` session before relaunching (the
  `opencode serve` process is left running and reused if it's already up on `OC_PORT`).

## 3. Watch the challenges

- `python3 tools/web_rush.py status --day N` — one line per challenge: first-pass steps
  plus agent state (`done` / `TIMED OUT` / `working` / `not dispatched`).
- Markers live in `workers/web/.rush-dNN/`: `brief-<chal>.md`, `firstpass-<chal>.json`,
  `<chal>.session` (the session id), `running-<chal>`, `done-<chal>`, `timedout-<chal>`.
- **Messaging an agent by hand:** attach tmux (`tmux -L huntress-web attach -t web`) and
  type directly into the pane - it's a live `opencode attach` view of the same session the
  driver talks to, so nothing needs the old Escape-first-picker-dismissal or
  `send-keys` tricks.
- **A challenge that looks stuck:** either type into its pane directly, or run
  `python3 tools/web_rush.py takeover --day N --challenges <chal>` to send a scripted
  nudge to the same session. There's no stall detector flagging anything automatically -
  this is purely "status says `working` for longer than seems right, go look."
- **The worker's loop** (what each pane is doing):
  `python3 tools/wk.py begin <mode>` → `submit <mode>` → `status` / `next`,
  single_solve → time_trial → mastery, serial within the family. If a target is
  crashed/burned and `begin <mode>` cannot recover it, the worker may use
  `python3 tools/wk.py reset <mode>`, then must `begin <mode>` again and
  re-derive every answer from the fresh target(s).
- **Check the GM directly (read-only):**
  `curl 'https://2026.huntress.ctf.games/?op=status&challenge=<route>' -H "Authorization: Bearer $HUNTRESS_GM_TOKEN"`.
- Runtime evidence (`start.json`, `targets.json`, `provisioning.json`, recon dumps,
  `answers.json`, `results.json`) lives in `workers/web/<base-id>/<mode>/` - git-ignored,
  disposable once the day is graded. The note and script go straight to
  `workers/web/solution-notes/day-NN-slug.md` + `workers/web/past-solutions/day-NN-slug-*.py`
  (git-tracked, symlinked into the worker dir, per the worker's task brief) - they are not
  scratch, they're the committed, taggable record (removed the older
  `solver_py_scripts/day-NN-web-exploitation/<slug>/` convention on 2026-10-28, folding
  everything that lived there - plus a handful of pre-2026-10-24 days whose only surviving
  `solve.py`/`NOTES.md` lived in `workers/web/<base-id>/` itself - into this one place, with
  day numbers corrected to the challenge's actual embedded/curriculum day via
  `workers/web/.events` rather than whatever day it happened to be solved on).
- **Shared scratch tmpfs can fill under concurrent slots.** Day 24 saw every Bash
  call (even `echo`) fail with ENOSPC on one worker's scratch tmpfs partway
  through a run with multiple slots active, then recover on its own after a
  pause with no code change. Non-Bash tools (Read/Write to project paths) kept
  working throughout. This is sandbox/infra contention, not a challenge or
  solver bug — if it recurs, wait and retry rather than debugging the solve.py.

## 4. After the night: merge the notes

Workers commit their own note + script. The coordinator then (~10 min):

1. Fold new bug-class methods into `docs/web-worker-lessons.md`.
2. Refresh `workers/web/solution-notes/INDEX.md` so the day/tag -> note ->
   script map includes the new note(s) and solver(s).
3. Fold the day's outcome + next-day prediction into `docs/lane-patterns.md`.
4. Fold any new `rec_*` recoverer into `solver_py_scripts/recovery_lib.py`'s
   `classify_and_recover` dispatch — after reviewing the graded evidence. This is what
   makes the next day's first pass stronger.
5. Append a dated entry to `docs/history.md`.
6. Commit the shared-doc updates (one commit per day is fine).
7. Clear that day's disposable runtime state once everything above is done and each
   worker's `workers/web/solution-notes/day-NN-slug.md` +
   `workers/web/past-solutions/day-NN-slug-*.py` are confirmed committed: remove
   `workers/web/<base-id>/` (evidence only - nothing is lost, the note/script never lived
   there) and `workers/web/.rush-dNN/`. Routine cleanup - there is no separate flattening
   pass needed under this convention, since workers write directly into the consolidated
   library rather than into a per-day directory that would later need folding in.

## 5. Standing rules (details in docs/policies.md)

- Web-exploitation routes only; never list, query, start, download or submit anything else.
- Untimed only; no timed, ranked, retry, practice or switch_target — there is no override
  in this pipeline. Workers may recover a crashed/burned target only through
  `tools/wk.py reset <mode>` (own family/mode, bounded), then must begin again and
  re-derive answers. Hall of Fame campaigns for web stay a coordinator decision outside
  this pipeline.
- The token is never printed, never sent to a target, and workers use it only through
  `tools/wk.py`.
- Never use, attach to, signal or kill the sibling repo's `-L huntress` socket. Kill the
  `web` session only when no driver is running.
- If the GM starts answering 429/5xx at scale, say so: the no-gating decision (2026-10-24)
  gets revisited and a shared lock comes back.
