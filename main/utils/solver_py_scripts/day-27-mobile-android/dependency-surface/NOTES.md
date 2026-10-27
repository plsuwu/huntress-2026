# Day 27 mobile - Dependency Surface

The "reachable dependency flaw from an exported surface" story is decoration (`DependencySurface` class,
`huntress.mechanic` = "reachable challenge-owned dependency" in the manifest). The answer is the plain v1 puzzle.

- Single-solve: v1 `codec:"base64"` (day 27), answered by `decode`'s `base64` branch.
- TT: **also v1** `base64` (days 23-26 shipped v2 `analysis` TT APKs; this day did not).
- Mastery: **11 sets, all v1** (no v2 `analysis` APK at all): days 1 plain, 4 xor, 7 caesar, 10 split, 11 xor,
  14 caesar, 18 xor, 20 dynamic, 21 reverse, set 10 = the d26 `.apks` (v1 `split` with the part in the feature
  split), set 11 = d27 base64. Day 8 still not seen as v2.

Graded: the rush first pass got single-solve, untimed TT and untimed mastery CORRECT on the first submit, with
**no code change**. Fixtures: `ctf-run/fixtures/mobile/d27-{single_solve,time_trial,mastery}.json` (artifacts went
through the fixtures symlink into the main checkout under `.../artifacts/`). Replay is 121/121 clean.
