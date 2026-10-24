# Active policies (2026-10-22)

- **Scope: web exclusively.** The user requested a web-only orchestrator and
  worker pipeline. List queries specify web; the strict route registry filters
  every row again. No other lane is assigned or queried.
- **No timed runs.** The user requested avoiding calls that spend timed runs.
  There is no override in this implementation. Timed-by-default and one-shot
  routes are held. Dual-pool routes are eligible only for untimed regular play.
  No retry/practice/reset/pair operation is implemented.
- **Initialization does not solve challenges.** Only read-only API inspection
  and offline tests were run during setup. Solves happen only through the
  release driver (`python3 tools/web_rush.py all --day N`) or a worker's
  `tools/wk.py`, under an authorized task.
- **One worker/session per challenge family.** The user noted that multiple web
  challenges release per day and requested challenge workers (2026-10-22).
  `web_rush.py --workers` bounds the pre-warmed slots (default 3; a day that
  releases more challenges than slots gets extra windows at release, fewer
  stands the spares down). Single-solve → time trial → mastery stays serial
  within each family, sharing one Claude session. Worker dirs are separate
  (`workers/web/<base-id>/`), submissions are bounded, and workers spawn no
  nested agents. The driver is a single process: `launch` refuses to run while
  the `web` session exists, and `all` kills the previous session first.
- **Workers own start and submit; the coordinator monitors (2026-10-23).** Workers
  receive the GM token and call the Game Master only through `tools/wk.py`, which
  validates scope, timing, prerequisites, exact
  answer keys, and current attempt identity before submission. The driver's
  automated first pass may also start attempts and submit when auto-recovery has
  every answer (untimed, and `start` idempotency makes the hand-off lossless);
  the human coordinator never submits on a worker's behalf.
- **Small endpoint load, no gating (2026-10-24).** GM requests are direct — no shared
  lock and no proactive pacing; workers are trusted not to flood the GM (solution
  speed over gating). Finite timeouts; transient 429/5xx/network failures are retried
  a bounded number of times with backoff, other HTTP errors are recorded as evidence.
  Reintroduce a shared lock if rate limits are hit.
- **Live target ownership.** Preparing targets are polled by status with a
  bounded wait. No worker may reset/retry/switch targets. A target-capacity
  conflict waits (bounded) for another family to finish; it never stops another family's site.
  An untimed target stops after 30 idle minutes, so the hand-off to the worker
  happens while the first pass's targets are still warm.
- **Release days are API metadata.** `day` selects release batches, not the
  curriculum number encoded in route ids. Challenge ownership is the full base
  id, including its slug, never the day alone.
- **No automatic deployment or ranked campaign.** Existing run-host keys and
  legacy tools are outside this pipeline. No run host has been configured.
- **Evidence over guesses.** Workers retain downloaded bytes, hashes, notes,
  requests/responses and reusable Python code. Only server responses establish
  grading. Partial acceptance triggers evidence-based corrections within a
  bounded worker budget.
- **tmux worker panes (2026-10-22).** At the user's request, workers run as interactive
  Claude panes on the private tmux socket `huntress-web` (session `web`), so this lane can
  run in parallel with, but isolated from, the sibling repository's `-L huntress` rush
  session. Never use, attach to, signal or kill that socket, and never share this lane's
  worker directories or state directories with it. Panes receive the GM token via
  `tmux -e` and use it only through `tools/wk.py` (it is never sent to a target); the
  pipeline never answers a trust/permission dialog for a worker.
- **Speed over ceremony, within the guards (2026-10-23).** At the user's direction the
  GM credential is low-stakes for this CTF and is given to workers, and the coordinator does
  not submit: its job is to monitor workers and take over from failed ones. Workers
  submit as soon as an answer is ready, chain modes at once, run with explicit
  `allowed_tools` allow rules, and may wait (bounded) for scoring confirmation or for another
  family to free a target slot. The guards live in `tools/wk.py`, not in who holds the token:
  untimed only, exact answer ids, matching attempt, bounded submissions, no
  retry/practice/reset/switch_target, own-family routes only. Allow rules are not a
  permission bypass; blanket rules and bypass modes remain rejected. The token is never
  sent to a target.
- **Rush-shaped pipeline (2026-10-24).** The lane runs the main pipeline's shape:
  `tools/web_rush.py` (all/prep/launch/watch/status/takeover) is the single driver —
  it prepares `workers/web/<base-id>/`, primes one Claude pane per slot on
  `-L huntress-web`, runs an automated untimed first pass per challenge at release
  (`solver_py_scripts/recovery_lib.py`), writes each brief and dispatches it, then
  watches stall markers. `tools/wk.py` is the worker's only GM door. The old
  config-driven coordinator (`web.py`, `orchcfg.py`, `config.toml`, planners,
  playbooks, tests) was torn out on 2026-10-24 and must not come back.
