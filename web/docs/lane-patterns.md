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
