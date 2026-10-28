# History

A dated log of each release night and campaign: what was released, what each lane needed, attempts spent and lost,
leaderboard results, tooling fixes. Newest at the bottom. Keep entries factual and short; lessons go to
grading-lessons.md, decisions to policies.md.

## 2026-10-22: initialize web-only pipeline

- Replaced the uninitialized multi-lane/Rust release flow with a Python
  coordinator, serial Claude worker jobs, exact answer validation and retained
  per-attempt evidence. No compiled checkout, git worktrees or run host required.
- Enforced web scope and untimed-only starts; disabled legacy mutation paths.
  Full queue, guard, artifact, worker, feedback and recovery tests use synthetic
  data and a fake local agent process.
- Read-only Python and curl list queries failed DNS resolution. No Game Master
  starts, submissions, timed attempts or real agent calls were made. Live shape
  validation remains a read-only preflight for a networked environment.
- Simplified the Nix development shell to Python/web tooling; removed missing
  ctf-run packaging. Configuration and operational docs now describe this flow.

## 2026-10-22: validate live metadata; workers per challenge

- Approved network-enabled GETs reached the GM. Web-filtered list returned
  24 routes / 8 families. A broad catalog GET was rejected by automatic approval
  review and was not executed; web-filtered reads supplied everything needed.
- Fixed the distinction between release day and curriculum_day/id number.
  Release days 19/20/21 contain 3/2/3 families, respectively. Added sanitized
  catalog and solved/unstarted status fixtures; queried OpenAPI read-only.
- Replaced the serial lane-wide queue with one Claude session per family,
  bounded to three concurrent families by default. Modes and feedback rounds
  remain serial per family; authenticated GM I/O is locked and rate limited.
- Added bounded target preparing/status polling, required assigned live targets,
  shared family notes/session continuation, cancellation, and retained capacity
  conflicts. No switch_target/reset/retry or timed override is available.
- Validated Day 21 with the live planner. No POSTs, new targets, submissions or
  real worker model runs were made. New target-ready/start behavior remains
  covered by synthetic tests, pending an authorized untimed solve.

## 2026-10-22: restored solution lessons and fresh-session handoff

- Reviewed restored Day 19/20 notes, focused solvers, unified recoverers and
  saved start JSON as local data. No historical entrypoint was executed and no
  new catalog, target, start or submission request was made.
- Added docs/web-worker-lessons.md and injected it into every worker prompt.
  Carried forward cumulative mixed-family mastery, question-to-target ID joins,
  proof-vs-flag distinction, randomized path derivation, session cookies,
  classifier priority and limits, request pacing and resolved service failures.
- Allowed workers to read historical .py/.md references via the explicit shared
  source directory, while preserving the rule against running old GM-facing
  CLIs or modifying shared sources. Work remains in each current job directory.
- Added docs/fresh-session.md with read-only discovery, single-route smoke test,
  whole-family/batch alternatives, automatic worker launch, monitoring and
  restart instructions. Existing 47 offline regression tests pass.

## 2026-10-23: worker-owned submission, supervised takeover
- Per user decision the GM token is low-stakes and given to workers: workers now start attempts and
  submit their own answers through `tools/wk.py` and chain modes themselves. The coordinator no longer
  starts or submits; it launches one worker per family, nudges, cross-checks GM status read-only, and takes
  over from failed workers (resumed session plus a brief). Guards moved into `tools/wk.py` / `ops.py`.
- Bounded waits for scoring-pending prerequisites and for the team live-target limit (cross-process
  wake-up through `workers/web/.events`); one request rate limit shared across processes by lock file.
- tmux pane backend on the private socket `huntress-web` (isolated from the sibling repo's `-L huntress`).
- 69 offline tests pass (real tmux with a fake interactive Claude); no live attempt or real model was run.

## 2026-10-24: web lane complete (days 06-13); two takeover patterns seen

- All 8 assigned families (day-06-reflect-upon-this through day-13-posted-too-much)
  completed single_solve, time_trial and mastery; every submission graded CORRECT.
  6 of 8 finished in a single uninterrupted worker session each.
- `web-exploitation-day-12-ask-again-later` single_solve took ~1h20m and two
  takeovers (pane exited cleanly, status 0, 18:25:17 and again 18:25:36) before
  resolving. Root cause: a blind, no-error-oracle second-order SQLi
  (`/submit-*` POST always returns 201 regardless of body; `/result-*` GET
  re-queries a stored field unescaped). The first worker pass tested candidate
  SQL-clause-themed field names (`filter`, `where`, `sql`, ...) only against
  an inert placeholder value, and tested real quote-breaking payloads only
  against content-word field names — never the combination of both — so an
  evidence-led but one-axis-at-a-time sweep of 100+ requests looked exhaustive
  without being so. A later pass ran field name x payload as a deliberate
  matrix and found `filter` + `' OR 1=1-- ` within minutes. time_trial and
  mastery for the same family then solved on the first try with the known
  pair. Carried forward to docs/web-worker-lessons.md.
- `web-exploitation-day-09-forged-with-care` and `day-10-uploading-expectations`
  each needed one takeover on an earlier launch, for an unrelated and
  unexplained reason: the captured pane showed an OpenAI-platform-style
  moderation refusal ("flagged for possible cybersecurity risk ... Daybreak
  access") immediately after the launch prompt was read, with zero tool
  activity, even though `tmux/launch.sh` for both explicitly passes
  `--model claude-sonnet-5`. Both were caught by the 900s idle timeout and
  resolved cleanly on relaunch with no recurrence; worth watching for if it
  reappears, since the cause (a provider/model mismatch between the launch
  command and the observed pane output) was not identified from local
  evidence alone.

## 2026-10-24: pipeline rebuilt rush-style; day 24 held for sign-off

- Tore out the old config-driven coordinator (`tools/web.py`, `orchcfg.py`,
  `config.toml`, planners, playbooks, tests) and rebuilt the lane as an adaptation
  of the main pipeline's `rush.py`: `tools/web_rush.py`
  (all/prep/launch/watch/status/takeover) + `tools/wk.py` + prompt templates
  (ADAPTATION-PLAN.md steps 1–3, committed).
- The 2026-10-24 user decision stands: no GM lock or pacing anywhere — direct
  calls, reactive 429/5xx backoff; reintroduce a shared lock if rate limits
  appear.
- Rehearsed read-only against day 23 (`watch --day 23 --rehearse --no-agents`):
  all 9 routes recognised CORRECT from GM status with zero starts, briefs
  written, no agents dispatched. Killed a stale old-pipeline `web` session left
  on the `huntress-web` socket.
- Docs rewritten for the new pipeline (`README.md`, `docs/runbook.md`,
  `docs/policies.md`); stale old-tooling references removed from
  `docs/lane-patterns.md` and `docs/live-web-catalog.md`.
- Day 24 is live. The rebuilt pipeline has not run against it yet: held until
  the user signs off on the pipeline state.

## 2026-10-24: Day 24 run, first use of the rebuilt pipeline; 2 families solved

- First live run of `web_rush.py all`. Added a `--model` override (default
  stays `claude-opus-5-5`) at user request to run this day's 3 pre-warmed
  workers on `claude-sonnet-5` instead; verified the generated `launch-N.sh`
  scripts picked it up before dispatch.
- Day 24 released 2 web-exploitation families (`day-14-the-api-has-opinions`,
  `day-15-fetch-quest`, curriculum days 14-15); slot 3 stood down as designed.
  Both completed single_solve/time_trial/mastery CORRECT in one session each,
  no takeovers needed.
- Two new bug classes graded CORRECT and folded into `recovery_lib.py`:
  `rec_ssrf` (fetch proxy reaching an internal-only route via
  `http://127.0.0.1:8080<secondary_route>`) and `rec_field_action_bypass` (an
  `/api/inventory`-disclosed fields x actions grid where `action=export`
  bypasses a check `action=view` enforces). Both verified against local HTTP
  fixtures before and after folding into the shared dispatcher. Full detail
  in `docs/web-worker-lessons.md` and the day's `NOTES.md` files.
- The driver's background monitor process exited quickly after dispatch for
  an unknown reason; dispatch itself was unaffected (confirmed via tmux
  window state and worker pane output). Status was tracked manually via
  `web_rush.py status --day 24` for the rest of the run. Worth a log file in
  a future iteration so this is diagnosable rather than just worked around.
- Both workers hit intermittent bare-socket GM timeouts (retried fine,
  `wk.py begin` is idempotent) and one worker's scratch tmpfs briefly filled
  under concurrent-slot load (self-recovered); neither affected grading.
  Both workers committed their own day directories before the session ended;
  this entry and the doc updates above are the coordinator's after-action
  pass per `docs/runbook.md` step 4. Tmux session `web` (socket
  `huntress-web`) killed after confirming both `agent: done`.

## 2026-10-25: Day 25 run; launcher `--add-dir` bug found and fixed pre-release; 3 families solved

- Relaunching `web_rush.py all --day 25 --workers 3 --model claude-sonnet-5`
  surfaced a real bug before release: `launcher_script()` built the
  `--add-dir` path list without the literal `--add-dir` flag, so Claude's own
  arg parser stopped reading flags at the first bare path — the real prime
  prompt, `--allowedTools`, `--settings` and `-n` never applied. All 3 workers
  came up confused ("what's my task?") instead of priming and replying
  "ready". Fixed (prepend `"--add-dir "` to the joined path string), killed
  the broken `web-driver`/`web` sessions, and relaunched cleanly with ~8
  minutes to spare before the 23:00 AEST release; all 3 slots primed
  correctly and replied "ready" on the second launch.
- Day 25 released 3 web-exploitation families (`day-16-shell-company`,
  `day-17-reset-expectations`, `day-18-resolver-revolt`, curriculum days
  16-18), confirming Day 24's prediction of sequential curriculum continuation.
  All 3 pre-warmed slots used, no stand-downs. All 3 completed
  single_solve/time_trial/mastery CORRECT in one uninterrupted worker session
  each, no takeovers, no stalls.
- Three new bug classes graded CORRECT and folded into `recovery_lib.py`:
  `rec_command_injection` (unescaped shell path parameter, `;`-chaining only
  — `$(...)` substitution is rejected server-side), `rec_reset_account_confusion`
  (password-reset token issued for a self-service account, redeemable against
  a different account id), `rec_graphql_field_bypass` (GraphQL list query
  hides a field by omission, matching single-object query does not check it
  at all). Also promoted `rec_sqli_group_close` (day-19's `') OR ('1'='1`
  payload) into the shared dispatcher on the strength of 5 independent
  confirmations across 3 release days. Full detail in
  `docs/web-worker-lessons.md` and the day's `NOTES.md` files.
- Striking cross-family consistency: all 3 families' mastery sets were
  independently exactly backup exposure + IDOR + group-closing SQLi + that
  family's own novel class — same 3 recurring classes, same proportions,
  all 3 families, same night. `recovery_lib.classify_and_recover` should now
  resolve 3 of any 4 future mastery targets automatically.
- Both the uncommitted `web_rush.py` fix and the doc/`recovery_lib.py` updates
  above are this entry's coordinator after-action pass per `docs/runbook.md`
  step 4, committed together. Tmux session `web` (socket `huntress-web`)
  and `web-driver` killed after confirming all 3 `agent: done` and GM status
  CORRECT on all 9 routes (3 families x 3 modes).

## 2026-10-27: Day 27 run, workers at `claude-sonnet-5` effort medium (one-off); 3 families solved

- Pre-release: a `web_rush.py` sanity probe (`all --day 27 --help`) actually ran
  the real pipeline for real — the script has no `--help` handling, so extra
  args after a recognized subcommand are silently ignored rather than
  rejected — launching a live 3-slot tmux session ~17 minutes early at the old
  hardcoded high effort. Caught before the 22:59:50 release-detection poll
  started (no GM/API calls had fired), killed, and relaunched cleanly with
  `web_rush.py all --day 27 --model claude-sonnet-5 --effort medium`. Added a
  proper `--effort` CLI override to `web_rush.py` (mirrors the existing
  `--model` override) so this stays a per-run flag, not a code default change.
- Day 27 released 3 web-exploitation families (`day-21-double-spent`,
  `day-22-socket-to-me`, `day-23-forwarded-misgivings`, curriculum days
  21-23), continuing the sequential curriculum pattern. All 3 pre-warmed slots
  used, no stand-downs. All 3 completed single_solve/time_trial/mastery
  CORRECT, each with a committed `NOTES.md` + reusable `solve.py` — no
  observed quality regression running workers at medium effort instead of the
  pipeline's default high.
- Three new bug classes graded CORRECT: `rec_double_spend` (non-atomic
  check-then-act race on a one-shot credit — serial touches permanently burn
  it, no in-app reset), `rec_unauthorized_event_stream` (WebSocket upgrade
  gated by an undisclosed query-parameter *name*, not a real role/auth check
  — needs non-stdlib `websockets`/`certifi`, so it lives in the new sibling
  `solver_py_scripts/recovery_lib_ws.py` rather than `recovery_lib.py`
  proper), and `rec_proxy_trust_bypass` (`X-Forwarded-For` trusted with no
  upstream-proxy allowlist). The first two folded into `recovery_lib.py`;
  full detail in `docs/web-worker-lessons.md` and the day's `NOTES.md` files.
- Mastery grew to **5** targets for the first time on two of the three
  families (`socket-to-me`, `forwarded-misgivings`): the standard backup +
  IDOR + group-closing SQLi plus day-20's LFI (newly cumulative in this slot)
  plus that family's own class, one target per class, no repeats.
  `double-spent` stayed at the standard 4 (no LFI yet).
- One coordinator-directed recovery at the user's explicit request:
  `double-spent`'s `single_solve` credit was already burned by an earlier
  serial touch before the worker reached it; the user called the GM's
  undocumented `op=reset` directly to re-provision a fresh target on the same
  attempt. Documented as a coordinator-only, human-directed path in
  `docs/policies.md` — never wired into `wk.py`/`web_rush.py`. The resulting
  stall (`stalled-day-21-double-spent.md`, no takeover file) self-resolved
  once the fresh target was in hand.
- This entry, the `docs/web-worker-lessons.md`/`docs/lane-patterns.md`/
  `docs/policies.md` updates, the `recovery_lib.py` fold-in, and the new
  `recovery_lib_ws.py` module are this entry's coordinator after-action pass
  per `docs/runbook.md` step 4, committed together. `workers/web/` cleaned
  after confirming all 3 `agent: done` and GM status CORRECT on all 9 routes
  (3 families x 3 modes).

## 2026-10-28: Migrated the agent from Claude Code to opencode

- At the user's request, replaced every Claude-Code-specific piece of the launch/messaging
  layer in `tools/web_rush.py` with opencode's client/server model: one shared
  `opencode serve` process hosts every challenge's session for the day; a tmux window per
  challenge runs `opencode attach <url> --session <id>` purely for human visibility/
  intervention; the driver's own turns go over `opencode run --attach <url> --session <id>
  --format json "<message>"`, never `tmux send-keys` (so the Escape-first picker-dismissal
  hack is gone too). `tools/wk.py` needed no changes - it was always pure Python/GM-client
  code with nothing Claude-specific in it.
- Dropped stall detection outright, at the user's direction: opencode has no hook mechanism
  equivalent to Claude's `Stop`/`PostToolUse`/`UserPromptSubmit` (confirmed against
  opencode's config schema - no hooks field), and since `opencode run` blocks until the
  agent's turn ends, "is it done" is just "has the call returned" - no marker-file
  heartbeat, no `monitor()` polling loop. `takeover` is now just "send one more message to
  the existing session" (or attach tmux and type into the pane directly - both are clients
  of the same live session, so nothing needs a fresh window or a pane-tail capture).
- Verified live before committing to this shape: (1) `opencode serve` + `opencode attach`/
  `opencode run --attach` really do let a human-visible tmux pane and the driver's
  programmatic calls share one session concurrently; (2) headless `ask` permissions are
  unreliable (hang indefinitely under `--attach`, auto-reject after ~15s under a fresh
  local server) but turned out unnecessary - plain bash tool calls already ran with zero
  permission friction under the environment's default policy in every test, so no
  permission flag or config was added (`--dangerously-skip-permissions` was tried and
  rejected - the auto-mode classifier itself flags it, and it's the kind of blanket bypass
  `docs/policies.md` already rejected on principle); (3) a single shared `opencode serve`
  process runs multiple sessions' agent turns genuinely in parallel (a 3-way concurrent
  batch took ~15.7s, about the same as one call alone at ~16.8s, not 3x) - confirming the
  new one-shared-server design doesn't cost the pipeline any of its worker concurrency.
- Model is left unset everywhere (`--model`/`--effort` on `web_rush.py` still override
  per-run) so workers track opencode's own configured default rather than a hard-coded
  string - a deliberate choice, since the old pipeline's history above shows the model
  string needed hand-editing or a CLI override more than once.
- Also fixed `AGENTS.md`, which had drifted to describe the sibling main pipeline's
  pre-2026-10-24 layout (`ctf-run`, `orchestrator/`, `utils/solver_py_scripts`,
  `attack_query.py`, the `previous-solution-research` skill) rather than this repo's own.

## 2026-10-28: Removed ADAPTATION-*.md, flattened workers/web/ into past-solutions/solution-notes

- `ADAPTATION-PLAN.md` and `ADAPTATION-REQUIREMENTS.md` were the design docs for the
  2026-10-24 rebuild; everything in them that was still-true runtime information had
  already migrated into `README.md`/`docs/runbook.md`/`docs/policies.md` (they were
  written from this plan in the first place) or into the code's own docstrings
  (`tools/web_rush.py`, `tools/wk.py`). Checked section by section before deleting;
  the one piece of unique value (the directory-layout tree) was re-added to `README.md`
  in a form that matches current reality rather than the pre-opencode-migration one it
  described. Removed both files and the live pointers to them in `AGENTS.md`/`README.md`;
  left the dated narrative mentions in this file's own 2026-10-24 entries alone - they
  describe what happened at the time, not current state.
- `workers/web/web-exploitation-day-{06..13}-*` and `day-19-trust-the-webhook` were the
  only surviving copies of `solve.py`/`NOTES.md` from a pre-2026-10-24 version of this
  pipeline that wrote solutions directly into the worker dir instead of into the
  git-tracked `solver_py_scripts/` - git-ignored, so at real risk of being lost outright.
  Rescued: every `solve.py` flattened verbatim into `workers/web/past-solutions/`
  (byte-identity verified before deleting the source), every mode's `NOTES.md` folded
  into one consolidated `workers/web/solution-notes/day-NN-slug.md` per day with YAML
  frontmatter (day/slug/modes/has_solve_py, mechanical fields only).
- At the user's direction, skipped deriving bug-class tags inline (originally planned) in
  favor of a `tags: []` placeholder plus `workers/web/solution-notes/TAGGING-PROMPT.md` -
  a self-contained prompt for a separate, cheaper model to fill in just that field, reading
  the consolidated notes and leaving everything else untouched.
- The remaining `web-exploitation-day-*` dirs (14-18, 20-23) held only disposable JSON
  runtime evidence with nothing to rescue - deleted along with them. Also removed dead
  files left over from the pre-2026-10-24 pipeline that nothing in the current code reads
  (`.lock`, `.gm.lock`, `.gm.last`, `coordinator-d22.log`) and stale per-day route
  caches/driver state for days already graded (`routes-d01.json`, `routes-d21.json`
  through `routes-d27.json`, `.rush-d23` through `.rush-d27`). `workers/web/.events` and
  `.slot-{1,2,3}` are untouched - still actively used by the current pipeline.
- `workers/web/` now holds only `past-solutions/`, `solution-notes/`, `.slot-{1,2,3}`,
  `.events`, and whatever the day in progress creates fresh (`<base-id>/`, `.rush-dNN/`,
  `routes-dNN.json`) - see the layout in `README.md`. `docs/runbook.md` §4 now says to
  clear a finished day's disposable state after folding its notes, rather than keeping it
  around indefinitely.

## 2026-10-28: Absorbed solver_py_scripts into workers/web/past-solutions + solution-notes

- At the user's direction, retired the `solver_py_scripts/day-NN-web-exploitation/<slug>/`
  write/read convention entirely (not just the pre-2026-10-24 orphaned days folded in
  earlier today). `solver_py_scripts/day-{05,19,20,21,22,24,25,26,27}-web-exploitation/`
  held every remaining past solution - extracted every `solve.py`/`NOTES.md` and deleted
  those directories; only `recovery_lib.py`, `recovery_lib_ws.py` and `web_templates/`
  (shared library code, not per-challenge solutions) remain in `solver_py_scripts/`.
- The real forensic work was day-number correction: `solver_py_scripts` directories were
  named by **release day** (when solved), not the challenge's own **embedded/curriculum**
  day (what `workers/web/.events` and every GM route id actually use), and the two
  disagreed repeatedly - `day-27-web-exploitation/forwarded-misgivings` is curriculum day
  23; `day-26-web-exploitation/trust-the-webhook` is curriculum day 19, the *same*
  challenge as an already-rescued orphaned note (its single_solve was solved under the
  pre-2026-10-24 pipeline, its time_trial/mastery much later) - merged rather than
  duplicated; `day-21-web-exploitation`/`day-22-web-exploitation` turned out to be
  independently-written, more condensed re-writeups of challenges already rescued
  earlier today from raw old data (days 06-10) - appended as a second section rather than
  creating competing files; days 01-05 used the web-exploitation lane's own original
  internal day counter (not the main event's) and had never touched `workers/web/`
  before. Verified every mapping against `.events` and the challenges' own route ids
  before moving anything, not guessed.
- `workers/web/solution-notes/` now holds one `day-NN-slug.md` per curriculum day, 01
  through 23, every past-solutions `.py` copy byte-identity-verified against its source
  before the source was deleted. Tag derivation (the frontmatter `tags` field) stayed out
  of scope for the automated pass per the user's direction - left as `tags: []` with
  `workers/web/solution-notes/TAGGING-PROMPT.md` as the handoff for a separate, cheaper
  model to fill in.
- Updated every live reference that pointed at the old paths: `tools/web_rush.py` now
  symlinks `workers/web/past-solutions/` and `workers/web/solution-notes/` into every
  worker/slot dir alongside the existing shared trees; `tools/prompts/{prime,task,task-done}.md`
  write new solutions directly into the consolidated library (and explain the YAML
  frontmatter/tagging convention) instead of a per-day `solver_py_scripts/` directory;
  `AGENTS.md`/`README.md`/`docs/runbook.md`/`docs/policies.md`/`docs/web-worker-lessons.md`/
  `docs/web-solver-templates.md`/`recovery_lib.py`'s own code comments, and several
  consolidated notes' internal cross-references to each other, all repointed to the new
  locations (and, where a comment had inherited the release-day/curriculum-day confusion
  above, corrected to the right day number while doing so). `.gitignore` now explicitly
  ignores the per-day runtime scratch (`.rush-d*/`, `routes-d*.json`, `web-exploitation-*/`)
  that used to be covered by a blanket `workers/web/` ignore, now that the consolidated
  library under the same root is deliberately git-tracked.

<!-- ## YYYY-MM-DD: Day N
- **Release** at HH:MM:SS. New lanes seen (not engaged): ...
- **First pass:** <lane>: CORRECT / needs <what>.
- **Agents:** <lane> fixed <what> by HH:MM; stalls and takeovers.
- **Merge and deploy:** tests N, replay N/N clean, deployed <hash>.
- **Campaign** (<plan file>, budget): N runs, results, #1 on X of Y; attempts lost and why. -->
