# Day 19 mobile - Reflection Maze

New codec name `"reflection"`, but no transform: `data` is the plain answer (`ANS-` + 16 hex), like day 15 `native`.
The "reflection maze" story (class/method names resolved at runtime) is decoration; the APK has no `lib/`, only the
manifest, `classes.dex`, the puzzle JSON and signature files. `lanes/mobile.rs::decode` has no `reflection` branch;
the generic search finds the code at depth 0. No code change.

Graded: the rush first pass got single-solve, untimed TT (1 APK) and untimed mastery CORRECT on the first submit.
Mastery (5 APKs) sampled d3 base64 (first time day 3 has appeared), d5 split, d11 xor (key 109), d18 xor (key 44),
d19 reflection. Fixtures: `ctf-run/fixtures/mobile/d19-{single_solve,time_trial,mastery}.json` (written through the
fixtures symlink into the main checkout, with the artifacts under `day-19-mobile-android/reflection-maze/artifacts/`).
Replay 90/90 clean.
