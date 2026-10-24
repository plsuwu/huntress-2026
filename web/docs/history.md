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

<!-- ## YYYY-MM-DD: Day N
- **Release** at HH:MM:SS. New lanes seen (not engaged): ...
- **First pass:** <lane>: CORRECT / needs <what>.
- **Agents:** <lane> fixed <what> by HH:MM; stalls and takeovers.
- **Merge and deploy:** tests N, replay N/N clean, deployed <hash>.
- **Campaign** (<plan file>, budget): N runs, results, #1 on X of Y; attempts lost and why. -->
