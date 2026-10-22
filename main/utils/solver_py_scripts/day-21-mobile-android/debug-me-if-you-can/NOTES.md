# Day 21 mobile - Debug Me If You Can

The anti-debug story ("cross the anti-debug state checks boundary", `DAY-21:anti_debug` string in `classes.dex`) is
decoration. `assets/huntress_puzzle.json` uses the old day 8 codec: `{"day":21,"codec":"reverse","data":"<answer reversed>"}`.
The APK has no `lib/` and no secondary DEX; `classes.dex` carries the usual decoy `client_secret=SECRET-...`.
No code change: `lanes/mobile.rs::decode`'s `reverse` branch answers it directly.

Graded: the rush first pass got single-solve, untimed TT (1 APK) and untimed mastery CORRECT on the first submit.
Mastery (5 APKs) sampled d3 base64, d10 split, d13 base64, d19 reflection and d21 reverse.
Fixtures: `ctf-run/fixtures/mobile/d21-{single_solve,time_trial,mastery}.json` (written through the fixtures symlink
into the main checkout, artifacts under `day-21-mobile-android/debug-me-if-you-can/artifacts/`). Replay 96/96 clean.
