# Day 31 mobile - Mobile Arena Gauntlet

The "mass-generated multi-app campaign that composes every released mobile primitive" story is decoration
(case id `MOB-31-...`, package `com.huntress.mobile.d31.v<hex>`). The answer is the usual `ANS-` + 16 hex.

- Single-solve: **v2 `analysis` for the first time** (day 31, `classes.dex` only; no v1 codec this day). The existing
  DEX interpreter answered it.
- TT: v2 `analysis` (day 31), `classes.dex` only; answered by the interpreter.
- Mastery: **15 bare v2 APKs** (no `.apks` set this time): days 1, 4, 6, 8, 10, 11, 15 + 16 (`libarena.so`),
  19, 20 (`assets/payload.dex`), 23, 25, 27, 30, and set 15 = d31. **Day 8 seen as v2 for the first time**
  (no new mechanism needed).

Graded: the automated first pass was CORRECT in all three modes with no code change. Fixtures:
`ctf-run/fixtures/mobile/d31-{single_solve,time_trial,mastery}.json` (written through the fixtures symlink into the
main checkout). Replay is 133/133 clean.
