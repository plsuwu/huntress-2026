# Day 18 warmups - Off the Rails

Encoding: **3-rail fence (zigzag) transposition** of the whole sentence ("Three rails. One very zigzag commute."):
plain characters, no hex step this time (the "<cipher> of hex" run ended). Blob = 109 B, no trailing newline.
Solved natively in ctf-run with no Python step.

- `codec::rail_fence_decode(t, rails)` (additive): top rail first, going down.
- `lanes/warmups.rs`: new `T::Rail(n)`. Blob peel tries rail counts 2..=12 (on the text as-is, without a trailing
  newline, and trimmed) right after the letter transforms.
- Mastery row 18 is pinned to `T::Rail(3)` in `fixed_codec`. **Not graded yet** (TT and mastery are ONE-SHOT,
  timed by default - coordinator only). Safety nets, used only if the pinned search finds nothing (`wide` retry in
  `solve_rows`): other rail counts 2..=12 on row 18, and padding appended after encoding (dash run or `-<digits>`).
- Fix found by a synthetic d18 mastery (d17's 83 blobs with the row-17 text split into Bacon row 17 + 3-rail row 18):
  once row 17 sits **inside the slug**, the classic-letter Bacon reading (`gbhagme`) fits any slug word, so every set was
  ambiguous. Pinned Morse/Bacon rows now use only their primary (hex) reading. Split points 1 char / 1/3 / 2/3 / last
  char all solve 83/83.
- First pass: REFUSED (`no slug after peeling`); deliveries were not saved in that run dir.

Graded (untimed, 2026-10-18): single-solve CORRECT on the first submit after the fix (`dapper-summit-brook-29`).
Route: single regular pool 1; `time_trial` (30 s) and `mastery` (105 s, 88 sets = 5N-2) are `rft=false`, ranked pool 1
(ONE-SHOT). Fixture: `ctf-run/fixtures/warmups/d18-single_solve.json`. Run dir: `ctf-run/runs-d18/warmups-day-18/`.
