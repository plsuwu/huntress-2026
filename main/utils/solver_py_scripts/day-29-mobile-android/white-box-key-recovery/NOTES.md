# Day 29 mobile - White Box Key Recovery

The "white-box table / clean-room table transformation" story is decoration (`WhiteBoxKeyRecovery` class,
`DAY-29:white_box_table`). The answer is the usual `ANS-` + 16 hex.

- Single-solve: v1 **new codec `"table"`**: `data` is hex; byte `i` = `answer[i] + key + 3*i` (mod 256), `key` = 41
  in the puzzle. `lanes/mobile.rs::untable` recovers offset and step from the known `ANS-` prefix (the four
  deltas must form an arithmetic progression) instead of hard-coding 3, and it also runs as a step of the generic
  search, so a renamed codec or a different step/offset is still found. The first pass refused (no branch).
- TT: v2 `analysis` (day 29); the existing interpreter answered it, no new API.
- Mastery: **13 sets, all v2**: days 1, 3, 7, 9, 10, 13, 15, 18, 19, 23, 24, set 12 = the d26 `.apks`
  (`base.apk` + split), set 13 = d29. **Day 3 seen for the first time** (no new mechanism needed).

Graded: after the `table` branch, single-solve, untimed TT and untimed mastery were all CORRECT on the first
submit. Fixtures: `ctf-run/fixtures/mobile/d29-{single_solve,time_trial,mastery}.json` (written through the
fixtures symlink into the main checkout). Replay is 127/127 clean.

If a later day changes the table into a real 256-entry S-box (a non-affine lookup), the `ANS-` prefix gives only
4 of the table's entries: look for the table in `classes.dex` (`encode`/`table` methods) or in the puzzle.
