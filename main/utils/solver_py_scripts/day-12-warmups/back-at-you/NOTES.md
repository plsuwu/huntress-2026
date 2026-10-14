# Day 12 warmups - Back at You (`warmups-day-12`)

Same slug-in-encoded-text template as days 10/11/13; the new layer is **string reversal** ("started at the wrong end").
Decoded: "Nice work decoding this warmup! To solve, submit this exact answer to the Game Master: <slug>".

## single_solve (DONE, CORRECT - flag 95fbde73...)
- `artifacts/blob_single.bin` is the whole text reversed; answer id `message`, slug `zesty-sunset-quartz-65`.
- `solve.py` = day-13 superset + `decode_reversed` (applied only when more English than input; also layers under codecs).

## time_trial (DONE, CORRECT - flag cc23c7ed..., 5.3 s of 30 s; NOT hall_of_fame_eligible)
- Route `run_for_time_supported: false`: timed by default, **only 1 ranked attempt** (`ranked_pool_size` 1,
  `unique_attempts_remaining` 0 after the first start). `--untimed` flag on run_timed.py is still required (see day-10/11).
- Whole-blob reversal again. Command: `python3 -I run_timed.py time_trial --challenge warmups-day-12 --untimed`.

## mastery (DONE via practice, flag b033549e...; the single timed attempt was burned -> "incorrect")
- 58 sets, 30 s deadline, one ranked attempt. First timed run answered wrong because of two bugs in the day-13 row solver:
  1. `solve_rows` did `.strip()` on rows, dropping the **leading space of a rot13 fragment** (row 02 `" qrpbqvat"`) ->
     "workdecoding" so the template regex never matched and fell to the junk fallback.
  2. Row 12 = `reverse(fragment)` **right-padded with `-` to 13 chars** (e.g. `89-enud-t----` -> `t-dune-98`). Pad dashes
     are indistinguishable from a genuine boundary hyphen, so `_match_template` tries each `-{2,}` run -> 0 or 1 dash and
     requires a *unique* full-template hit.
- Row encodings (fixed per index, as day 13): 01 b64, 02 rot13, 03 hex, 04 binary, 05 decimal, 06 octal, 07 %-hex,
  08 &#dec;, 09 "\uXXXX", 10 b32, 11 b64url, 12 reversed+dash pad. Fragment widths vary per set (8-9 chars).
- Finish with `python3 -I run_timed.py mastery --challenge warmups-day-12 --untimed --practice` (added `--practice`; reuses
  the last artifact, untimed). Fixtures: `artifacts/mastery-2756bb7f` (timed), `mastery-9cb70287` (practice, same blobs).
- Offline regression run for this solver: all 58 fixture sets -> template-matching slugs, plus 189/189 day-13 fixtures.

## Lessons for future agents
- Before a one-shot timed start on a `run_for_time_supported:false` route (pool 1), the *first* start is the fixture
  collection; make the row parser whitespace-faithful and tolerant of padding, since the single/time-trial blobs
  don't reveal mastery row quirks. `practice` recovers a burned attempt.
