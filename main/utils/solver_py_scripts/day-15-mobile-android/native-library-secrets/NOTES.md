# Day 15 mobile - Native Library Secrets

Solved by the existing decoder (no code change). `assets/huntress_puzzle.json` has `"codec":"native"` with the answer
in `data` verbatim; the APK also ships `lib/x86_64/libarena.so` carrying the same `ANS-...` string next to
`huntress_arena_secret` / `huntress_arena_day` symbols (not needed - the JSON wins; if a future day blanks `data`, read
the string out of the `.so`). Answer validated by `^ANS-[0-9A-F]{16}$`.

Mastery (5 APKs) now **samples earlier days' codecs**, each APK tagged with its `day`: d1 plain, d7 caesar(key 15, on the
whole string incl. `ANS-`), d10 split(parts/order), d14 caesar(key 4), d15 native. TT = one d15 native APK.
All three modes CORRECT first try. Route: dual-pool, ranked 32 / regular 32; TT 50 s, mastery 170 s.
