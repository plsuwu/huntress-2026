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
  releases more challenges than slots gets extra sessions at release, fewer
  stands the spares down). Single-solve → time trial → mastery stays serial
  within each family, sharing one opencode session. Worker dirs are separate
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
- **tmux worker panes (2026-10-22; opencode since 2026-10-28).** At the user's request, workers
  are visible as interactive panes on the private tmux socket `huntress-web` (session `web`),
  so this lane can run in parallel with, but isolated from, the sibling repository's
  `-L huntress` rush session. Never use, attach to, signal or kill that socket, and never share
  this lane's worker directories or state directories with it. Each pane runs
  `opencode attach <url> --session <id>` against the shared `opencode serve` process the driver
  starts (its own environment carries the GM token - tool execution happens server-side, so no
  per-pane token flag is needed, and it is never sent to a target); the pipeline never answers a
  trust/permission dialog for a worker.
- **Speed over ceremony, within the guards (2026-10-23; no permission bypass since 2026-10-28).**
  At the user's direction the GM credential is low-stakes for this CTF and is given to workers,
  and the coordinator does not submit: its job is to monitor workers and take over from failed
  ones. Workers submit as soon as an answer is ready, chain modes at once, and may wait (bounded)
  for scoring confirmation or for another family to free a target slot. The guards live in
  `tools/wk.py`, not in who holds the token: untimed only, exact answer ids, matching attempt,
  bounded submissions, no retry/practice/reset/switch_target, own-family routes only. No
  permission flag or allow-list is passed to opencode either - confirmed live that plain bash
  tool calls already run with zero friction under the environment's default policy, and
  `--dangerously-skip-permissions` is both unneeded and a blanket bypass, which this pipeline
  already rejected on principle before opencode existed here. The token is never sent to a
  target.
- **Undocumented `op=reset` is coordinator-only, human-directed (2026-10-27).**
  The GM exposes an undocumented per-challenge `op=reset`
  (`POST / {"op":"reset","challenge":"<id-or-id/mode>"}`) that tears down a
  live target and re-provisions a fresh one on the same attempt, resetting
  in-app state (e.g. a one-shot credit back to its starting value). It is not
  in `AGENTS.md`'s operations table and has no code path in `wk.py` or
  `web_rush.py` — the standing no-reset/retry/switch_target rule for workers
  is unchanged. Used once, at the user's explicit direction, to recover
  day-27 double-spent's `single_solve` attempt after its one-shot credit was
  burned serially before the worker session reached it. Treat it as a
  one-off coordinator recovery action for a specific stuck attempt, never as
  a routine or automated path, and do not wire it into `wk.py`/`web_rush.py`.
- **Rush-shaped pipeline (2026-10-24).** The lane runs the main pipeline's shape:
  `tools/web_rush.py` (all/prep/launch/watch/status/takeover) is the single driver —
  it prepares `workers/web/<base-id>/`, primes one opencode session per slot (visible via a
  `tmux attach` window on `-L huntress-web`), runs an automated untimed first pass per
  challenge at release (`solver_py_scripts/recovery_lib.py`), writes each brief and dispatches
  it. `tools/wk.py` is the worker's only GM door. The old config-driven coordinator (`web.py`,
  `orchcfg.py`, `config.toml`, planners, playbooks, tests) was torn out on 2026-10-24 and must
  not come back.
- **Opencode migration, no stall detection (2026-10-28).** The lane moved from Claude Code to
  `opencode`: one shared `opencode serve` process hosts every challenge's session for the day;
  the driver's own turns go over `opencode run --attach <url> --session <id>`, which blocks
  until the agent's turn ends - that block is the entire "is it done" signal. This replaces the
  old Claude-Code-specific Stop/PostToolUse/UserPromptSubmit hook machinery and its
  `monitor()`/stalled-`<chal>`.md escalation outright, at the user's direction: stall detection
  was judged not worth keeping, and opencode has no hook mechanism to build it from anyway
  (confirmed against opencode's own config schema). `takeover` now just sends one more message
  to a challenge's existing session; attaching tmux and typing into the pane directly works
  just as well, since both are clients of the same live session. Confirmed live before this
  decision: a single shared `opencode serve` runs multiple sessions' turns genuinely in
  parallel, not serialized, so this does not cost the pipeline any of its concurrency. Model is
  left unset everywhere so workers track opencode's own configured default rather than a
  hard-coded string, per the user's explicit call.
