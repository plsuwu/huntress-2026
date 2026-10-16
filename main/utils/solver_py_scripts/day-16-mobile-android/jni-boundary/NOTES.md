# Day 16 mobile - JNI Boundary

Solved by the existing decoder (no code change); the rush first pass got all three modes CORRECT via untimed attempts.
Same shape as day 15: `assets/huntress_puzzle.json` has `"day":16,"codec":"native"` with the answer in `data` verbatim,
and `lib/x86_64/libarena.so` carries the same `ANS-...` string beside `huntress_arena_secret` / `huntress_arena_day`
(not needed - the JSON wins). The "JNI boundary" story is decoration.

Mastery (5 APKs) samples earlier days' codecs again: d1 plain, d4 xor(key 21, base64), d11 xor(key 232, base64),
d13 base64, d16 native. TT = one d16 native APK. Fixtures: `ctf-run/fixtures/mobile/d16-{single_solve,time_trial,mastery}.json`,
artifacts under `artifacts/`.
