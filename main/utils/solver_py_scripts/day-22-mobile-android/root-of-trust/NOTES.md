# Day 22 mobile - Root of Trust

The root-detection story (`DAY-22:root_detection:synthetic root detection` string in `classes.dex`) is decoration.
`assets/huntress_puzzle.json` uses the old day 9/13 codec: `{"day":22,"codec":"base64","data":"<base64 of the answer>"}`.
The APK has no `lib/` and no secondary DEX; `classes.dex` carries the usual decoy `client_secret=SECRET-...`.
No code change: `lanes/mobile.rs::decode`'s `base64` branch answers it directly.

Graded: the rush first pass got single-solve, untimed TT (1 APK) and untimed mastery CORRECT on the first submit.
Mastery now has **6 APKs** (5 on days 20-21). It sampled d1 `plain` (first time seen: `data` is the answer as-is,
found by the generic search at depth 0), d7 caesar (key 15), d12 hex, d13 base64, d17 indexed and d22 base64.
Fixtures: `ctf-run/fixtures/mobile/d22-{single_solve,time_trial,mastery}.json` (written through the fixtures symlink
into the main checkout, artifacts under `day-22-mobile-android/root-of-trust/artifacts/`). Replay 99/99 clean.
