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

## Day 27 outcome (2026-10-27)

- **Prediction from Day 25 confirmed (via Day 26's curriculum 19-20):**
  curriculum continued sequentially again — `double-spent` (curriculum 21),
  `socket-to-me` (curriculum 22), `forwarded-misgivings` (curriculum 23); 3
  families, all 3 pre-warmed slots used, no stand-downs.
- **One-off experiment: workers ran `claude-sonnet-5` at `--effort medium`**
  (pipeline default is `claude-sonnet-5` at high effort; `web_rush.py` gained
  an `--effort` CLI override, mirroring the existing `--model` override, so
  this stays a per-run flag rather than a code default change). All 3
  families still finished single_solve/time_trial/mastery CORRECT with full
  NOTES.md + reusable solve.py write-ups per family — no observed quality
  regression at medium effort on this release's bug classes.
- **Operational near-miss, no live impact:** an exploratory `web_rush.py all
  --day 27 --help` probe actually ran the real `all` pipeline for real (the
  script has no dedicated `--help` handling — any extra args after `all` are
  silently ignored, not rejected), launching a live 3-slot tmux session ~17
  minutes early at the old hardcoded high effort. Caught and killed before
  the 22:59:50 release-detection poll started, so no GM/API calls had fired
  yet; relaunched cleanly with explicit `--day 27 --model claude-sonnet-5
  --effort medium`. **Lesson:** this driver has no safe no-op/dry-run flag —
  treat *any* invocation of `all`/`launch`/`watch` as live, never as a way to
  read the script's own usage text.
- Three new bug classes this release (now in `recovery_lib.py` as
  `rec_double_spend`, `rec_proxy_trust_bypass`, and in the new sibling
  `recovery_lib_ws.py` as `rec_unauthorized_event_stream` — see
  `web-worker-lessons.md`): a non-atomic check-then-act race that double-spends
  a one-shot credit (strictly one-shot per target — any serial touch burns it
  with no in-app reset); an unauthorized WebSocket event-stream subscription
  gated by an undisclosed query-parameter *name* rather than any real
  role/auth check; and trusted-reverse-proxy header spoofing (`X-Forwarded-For`
  trusted with no upstream allowlist). The WS class needs non-stdlib
  `websockets`/`certifi`, hence the new sibling module rather than folding it
  into `recovery_lib.py` proper.
- **Mastery grew to 5 targets for the first time** (`socket-to-me`,
  `forwarded-misgivings`): the standard backup exposure + IDOR +
  group-closing SQLi **plus day-20's LFI, newly cumulative in this slot**,
  plus that family's own novel class — one target per class, no repeats.
  `double-spent`'s mastery stayed at the standard 4 (no LFI yet). Don't
  hard-code an expected target count; always read `questions`/`targets`.
- **One coordinator-directed recovery, at the user's explicit request:**
  `double-spent`'s `single_solve` credit had already been burned by a
  serial touch before the worker reached it (no in-app recovery exists); the
  user used the GM's undocumented `op=reset` directly to re-provision a fresh
  target on the same attempt. Now documented in `docs/policies.md` as a
  coordinator-only, human-directed path — never wired into `wk.py`/
  `web_rush.py`. The stall this produced (`stalled-day-21-double-spent.md`,
  no takeover file) self-resolved once the fresh target was in hand; no
  `web_rush.py takeover` was needed.
- **Prediction:** curriculum days 21-23 seen; expect release day 28 to
  continue sequentially from curriculum day 24. Expect mastery sets to keep
  growing (4 -> 5 targets this release) rather than staying fixed at 4 —
  check the actual target count every time. `recovery_lib.classify_and_recover`
  should now resolve 4 of any 5 mastery targets automatically (backup + IDOR
  + SQLi + LFI) via the first pass for any family whose cumulative pool has
  caught up to day-27's, leaving only that family's own class for the worker.
