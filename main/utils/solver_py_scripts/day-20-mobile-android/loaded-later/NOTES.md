# Day 20 mobile - Loaded Later

New codec `"dynamic"`: `assets/huntress_puzzle.json` is `{"day":20,"codec":"dynamic","data":"assets/payload.dex"}`.
`data` is a **path to another APK member**, not an encoded string. `assets/payload.dex` is a small secondary DEX
(`Lcom/huntress/arena/payload/Module;`) that holds the answer as a plain string-table entry (`ANS-` + 16 hex), next to
the strings `ANSWER`, `CONTRACT` and `DAY20-SIGNED-MODULE:secondary DEX loading`. `classes.dex` has a decoy
`client_secret=SECRET-...` and no `ANS-`. The APK has no `lib/`.

Fix (`lanes/mobile.rs::solve_apk`): any string value in the puzzle that names an APK member is read, and a single
distinct `ANS-` code found there is the answer. 0 codes falls through to `decode`. More than one code is an error.
If `decode` fails, a unique `ANS-` code in `lib/*.so` is used (the day 16 prediction; not needed yet). Both paths
refuse to guess when several codes appear.

The first pass REFUSED single-solve (generic search: no code in the string `assets/payload.dex`). After the fix,
single-solve, untimed TT (1 APK) and untimed mastery were all CORRECT on the first submit. Mastery (5 APKs) sampled
d2 reverse (first time day 2 has appeared), d9 base64, d11 xor (key 41), d18 xor (key 100) and d20 dynamic.
Fixtures: `ctf-run/fixtures/mobile/d20-{single_solve,time_trial,mastery}.json`. The artifacts were written through the
fixtures symlink to the main checkout, under `day-20-mobile-android/loaded-later/artifacts/`. Replay 93/93 clean.

Next: if a later payload is encoded (e.g. xor/base64 of the DEX, or a `key` alongside the path), feed the member's
bytes through `generic` instead of only scanning them for `ANS-`.
