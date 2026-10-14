# Keystore Misuse (day 13, mobile-android-day-13-keystore-misuse)

Same tiny hand-built-APK family as day-04..11 (see day-10/11 NOTES.md). `assets/huntress_puzzle.json` holds the answer;
the "app-controlled wrapping key / nominal Keystore" story is decoration — no decompiler needed.
Single-solve codec = `base64`: `{"codec":"base64","data":"<b64 of ANS-...>"}`. Existing handler in solve.py covered it.
`solve.py` / `mastery.py` are copies of day-11 with only `CHALLENGE` changed.

## Layout
- `solve.py <apk>` / `solve.py --submit [/time_trial]`; run with `TMPDIR=$PWD/artifacts/<mode>`, needs `HUNTRESS_GM_TOKEN`.
- `mastery.py [/mastery]` batched mastery (5 sets, bare APKs, one submit).
- `artifacts/<mode>/` start fixtures + APKs.

## Results (2026-10-14) — all three modes CORRECT on the first try, no retries spent
- single_solve flag `2553434d2c3082ec36bde024d2973aa2`
- time_trial flag `70525c29ab3ff765b4297f4590fca1f4` — REGULAR (untimed) attempt, `hall_of_fame_eligible:false`
- mastery flag `2478403e54f4b790d4eae9b421ca78e4` — REGULAR (untimed), 5/5 answered, `hall_of_fame_eligible:false`

## Gotcha (differs from day 10/11)
- `run_for_time_supported` is **true** on time_trial/mastery: a plain `start` is a regular untimed attempt (deadline null)
  and earns the flag/points but is NOT Hall-of-Fame eligible. For a ranked run send `"run_for_time":true` on
  start/retry (ranked window: time_trial 50 s, mastery 110 s; ranked pools 16 / 8). Ranked runs were not attempted here;
  after the regular solve, `retry` with `run_for_time:true` would be needed (16 / 8 ranked attempts remained).
