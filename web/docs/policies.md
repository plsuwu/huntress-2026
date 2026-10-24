# Active policies (2026-10-22)

- **Scope: web exclusively.** The user requested a web-only orchestrator and
  worker pipeline. List queries specify web; the strict route registry filters
  every row again. No other lane is assigned or queried.
- **No timed runs.** The user requested avoiding calls that spend timed runs.
  There is no override in this implementation. Timed-by-default and one-shot
  routes are held. Dual-pool routes are eligible only for untimed regular play.
  No retry/practice/reset/pair operation is implemented.
- **Initialization does not solve challenges.** Only read-only API inspection
  and offline tests were attempted during setup. Future solves require an
  explicit `run --execute` or `watch --execute` command from an authorized task.
- **One worker/session per challenge family.** The user noted that multiple web
  challenges release per day and requested challenge workers (2026-10-22).
  `[agents].workers` bounds concurrent families (default 3, supported 1–8).
  Single-solve → time trial → mastery stays serial within each family, sharing
  one Claude session. Jobs use separate directories, finite budgets and no
  nested agents. A coordinator lock prevents overlapping local queue drivers.
- **Workers own start and submit; the coordinator monitors (2026-10-23).** Workers
  receive the GM token and call the Game Master only through `tools/wk.py`, which
  validates scope, timing, prerequisites, exact
  answer keys, and current attempt identity before submission.
- **Small endpoint load.** GM requests from every process are serial, separated by
  0.5 seconds (a shared lock file), with finite timeouts. HTTP errors are recorded without automatic
  replay. An ambiguous POST is reconciled through status on the next invocation.
- **Live target ownership.** Preparing targets are polled by status with a
  bounded wait. No worker may reset/retry/switch targets. A target-capacity
  conflict waits (bounded) for another family to finish; it never stops another family's site.
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
  session. Never use, attach to, signal or kill that socket; never share its worktrees,
  state directories or lock. Panes get no GM credentials and the pipeline never answers a
  trust/permission dialog for a worker. `--backend print` is the fallback.
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
