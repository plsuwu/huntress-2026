# Lane patterns and per-day predictions

How each lane evolves from day to day, written by the lane agents after each release and read by them before the
next one. A good prediction lets an agent pre-support tomorrow's twist before the release.

## Web lane

Live list/status/OpenAPI metadata was inspected read-only on 2026-10-22; see
docs/live-web-catalog.md. Eight families span release Days 19–21, with curriculum
numbers 01–08. No target pages or newly started assignments were inspected.

The restored historical solutions now supply graded observations for curriculum
01–05. Read [web-worker-lessons.md](web-worker-lessons.md) for the worker brief
and source map. Mastery is cumulative across earlier bug types: inspect every
target, rather than using only the method named by the worker's challenge route.
Known earlier families are backup exposure, IDOR, SQLi, PHP LFI and Django
per-route authorization bypass. Admin clues must be distinguished before generic
API/IDOR signs; a form alone is insufficient to classify a new target as SQLi.

The web lane uses assigned live sites in targets[], with no downloadable
artifacts. Workers must document observed formats and exploit families in each
challenge's NOTES.md; summarize reusable patterns here after genuine grading evidence.
Mode names do not establish timing: all non-single modes are dual-pool and are
played untimed only; this pipeline has no timed path at all. See
docs/live-web-catalog.md for the live catalog/status contracts and the observed
ready-response shape (active; targets have id/url). No new targets were started
for this review.

## Per-lane summary

<!-- ### <lane>: <one-line character>
- Single/TT: what the artifact is and what is asked.
- Mastery/multi-set: how sets are built.
- Day-by-day pieces: d1 ..., d2 ..., ...
- Gotchas: ...
- Prediction for the next day: ... -->

## Day 24 outcome (2026-10-24)

- Rush-shaped `web_rush.py all --day 24 --workers 3 --model claude-sonnet-5`
  (one-off model override, per user request — pipeline default is
  `claude-opus-5-5`). Day 24 was already past its 23:00 AEST release when the
  driver started, so release detection fired on the first poll.
- Only 2 web-exploitation families released
  (`day-14-the-api-has-opinions`, `day-15-fetch-quest`): 2 of 3 pre-warmed
  slots got a challenge, slot 3 stood down automatically (fewer challenges
  than slots, as designed). Both finished single_solve, time_trial and
  mastery CORRECT in one uninterrupted session each, no takeovers.
- Two new bug classes this family introduced (now in
  `recovery_lib.py`/`rec_ssrf`, `rec_field_action_bypass` — see
  `web-worker-lessons.md`): SSRF via a fetch proxy reaching an internal-only
  route over loopback, and a field/action authorization-check grid that
  leaks a protected field under one action (`export`) but not another
  (`view`). Both mastery sets were cumulative as expected (backup exposure +
  IDOR in both, plus day-19's SQLi for `fetch-quest`'s mastery).
- The driver's background stall-monitor exited within ~25s of dispatch for
  an unexplained reason (dispatch itself, including the stand-down of slot
  3, completed correctly first) — no stdout was captured since the tmux pane
  closed with the process. Not reproduced or root-caused; worth capturing to
  a log file (not just the tmux pane) next time so a recurrence is
  diagnosable instead of just worked around by manual `status` polling.
- **Prediction:** with only 2 of the day's families seen (curriculum days
  14-15), expect the next web release to continue sequentially from
  curriculum day 16; no new structural twist observed in list/status
  metadata beyond the two bug classes above.

## Day 25 outcome (2026-10-25)

- **Prediction from Day 24 confirmed:** curriculum continued sequentially —
  day-16-shell-company, day-17-reset-expectations, day-18-resolver-revolt (3
  families, all 3 pre-warmed slots used, no stand-downs).
- **Launcher bug found and fixed before release:** `web_rush.py`'s
  `launcher_script()` built the `--add-dir` paths without the literal
  `--add-dir` flag token, so Claude's own arg parser stopped reading flags at
  the first bare path — the real prime prompt, `--allowedTools`, `--settings`
  and `-n` never applied, and every worker came up asking "what's my task?"
  instead of reading `prime.md`/replying "ready". Fixed at
  `tools/web_rush.py` (prepend `"--add-dir "` to the joined path list) and
  reconfirmed clean on a full relaunch — all 3 slots primed correctly and
  replied "ready" before 23:00. **Lesson:** after any change to
  `launcher_script`/`agent_settings`, a `--rehearse` or at least a `launch`
  + manual pane check before the real release would have caught this without
  burning the live release window — the first (broken) launch this night
  used real wall-clock minutes before 23:00 that a rehearsal check could
  have protected.
- Three new bug classes this release (now in `recovery_lib.py` as
  `rec_command_injection`, `rec_reset_account_confusion`,
  `rec_graphql_field_bypass` — see `web-worker-lessons.md`): OS command
  injection via an unescaped shell-concatenated path parameter (`;`-chaining
  only, `$(...)` substitution is rejected server-side); password-reset
  token/account-id confusion (request a token for your own self-service
  account, redeem it against a different account id); GraphQL field-level
  authorization bypass (list query hides the sensitive field, matching
  single-object query does not check it at all).
- **Striking cross-family consistency:** all 3 families' mastery sets were
  *exactly* backup exposure + IDOR + day-19's group-closing SQLi (`')
  OR ('1'='1`, working unmodified on first try every time, zero fallback
  payloads needed) + that family's own novel class — the same 3 recurring
  classes, in the same proportion, independently in all 3 families on the
  same night. `rec_sqli_group_close` is now promoted into `recovery_lib.py`
  on the strength of 5 independent confirmations across 3 release days.
  Backup exposure + IDOR + group-closing SQLi should be treated as the
  default expected first three mastery targets going forward.
- All 3 families finished single_solve, time_trial and mastery CORRECT in
  one uninterrupted worker session each, no takeovers, no stalls.
- **Prediction:** curriculum days 16-18 seen; expect release day 26 to
  continue sequentially from curriculum day 19 (verify against the actual
  `curriculum_day`/family title in day-26 clues rather than assuming — don't
  confuse curriculum-day numbering with release-day numbering, they are
  different sequences). Expect mastery on any new family to again mix
  backup exposure + IDOR + group-closing SQLi with one new class —
  `recovery_lib.classify_and_recover` should now resolve 3 of any 4 mastery
  targets automatically via the first pass, leaving the worker to focus on
  classifying only the unrecognized one.
