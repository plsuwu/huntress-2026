# Huntress 2026 web solver pipeline

The `web-exploitation` lane's solution pipeline, rebuilt 2026-10-24 as an adaptation of
the main pipeline's `rush.py` (one Python driver, direct Game Master calls, tmux as the
only UI) and migrated 2026-10-28 from Claude Code to opencode. The previous config-driven
coordinator was torn out as ineffective; `ADAPTATION-PLAN.md` records the adaptation and
`docs/runbook.md` is the release-night runbook.

Requires Python 3.11+, the `opencode` CLI and `tmux`. No Python dependencies, no Rust, no
SSH, no run host.

## Components

| Path | Role |
|---|---|
| `tools/web_rush.py` | The driver: `all\|prep\|launch\|watch\|status\|takeover [--day N] [--rehearse] [--workers N]`; also starts/talks to the shared `opencode serve` process |
| `tools/wk.py` | The worker's only GM door: `begin\|submit\|status [--live]\|next`, guarded (untimed only, own family only) |
| `tools/prompts/` | `prime.md` (pre-release warm-up), `task.md` / `task-done.md` (brief tails) |
| `solver_py_scripts/recovery_lib.py` | Shared target classifiers/recoverers — the automated first pass and the worker solvers both use it |
| `workers/web/` | Runtime state, git-ignored: per-challenge dirs, `.rush-dNN/` markers, sessions and briefs, route caches |

## Release night

```sh
tmux new -d -s web-driver "python3 -u tools/web_rush.py all --day N"   # ~22:45 AEST, from the repo root
tmux attach -t web-driver                                             # the driver's log
tmux -L huntress-web attach -t web                                    # the worker panes (opencode attach, live)
python3 tools/web_rush.py status --day N                              # first pass + agent state
```

At the 23:00 AEST release the driver discovers the day's web challenges, runs an
automated untimed first pass per challenge (`single_solve` → `time_trial` → `mastery`,
each started only once its prerequisite is CORRECT), writes a brief per challenge and
dispatches it to one primed opencode worker session per challenge (tmux socket
`-L huntress-web`, session `web`, one `opencode attach` window per session for human
visibility/intervention; more challenges than slots get extra sessions, fewer stand the
spares down). Every worker session lives on one shared `opencode serve` process the
driver starts - the driver's own turns go over `opencode run --attach ... --session ...`,
never tmux keystrokes, and that call blocking until the agent's turn ends is the whole
"is it done" signal (no stall-detection hooks - see `docs/policies.md`). The worker solves
the modes through `tools/wk.py`, writes
`solver_py_scripts/day-NN-web-exploitation/<slug>/{solve.py,NOTES.md}` and commits it.
`web_rush.py takeover --day N --challenges <chal>` sends one more message to a
challenge's existing session (nothing is lost - `wk.py begin` reconnects to the same
attempt); attaching tmux and typing into the pane directly works just as well.

## Scope and guards

- **Web only.** The driver and the workers touch `web-exploitation-*` routes and nothing
  else — never list, query, start, download or submit any other lane.
- **Untimed only.** No timed, ranked, retry, practice, reset or switch_target — there is
  no override in this pipeline; plain starts on dual-pool routes are free untimed regular
  attempts. GM calls are direct (no lock, no pacing — 2026-10-24 decision) with reactive
  429/5xx backoff; a shared lock comes back if rate limits appear.
- **The token stays guarded.** Workers hold `HUNTRESS_GM_TOKEN` (low-stakes, per user
  decision) and use it only through `tools/wk.py`; it is never printed and never sent to
  a target. Workers never get the repo root as cwd or `--add-dir` — the jumpbox SSH key
  and `.env` live there.
- **Evidence over guesses.** Only GM responses establish grading. Workers retain
  evidence and notes; the coordinator folds new recoverers into `recovery_lib.py` and
  the lessons into `docs/web-worker-lessons.md` after reviewing graded evidence.

Read `AGENTS.md` and `HOMEBASE.md` for the CTF overview, `docs/policies.md` for the
standing rules, `docs/web-worker-lessons.md` for the accumulated curriculum knowledge
(injected into every worker prompt), and `docs/lane-patterns.md` + `docs/history.md`
for the per-day records.
