# Cleartext Session (day 08, mobile-android-day-08-cleartext-session)

Same tiny hand-built-APK family as day-04..07 (see day-07 NOTES.md). `assets/huntress_puzzle.json` holds the answer;
the "cleartext traffic / replay against backend" story is decoration — no network, no decompiler needed.
Single-solve codec = `reverse`: `{"codec":"reverse","data":"6667D24E8BC6DE39-SNA"}` -> reversed -> `ANS-93ED6CB8E42D7666`.

## Layout
- `solve.py <apk>` / `solve.py --submit [/time_trial]` (copied from day-07; added explicit `reverse` codec; the
  generic BFS already covered reverse). Run with `TMPDIR=$PWD/artifacts/<mode>`, needs `HUNTRESS_GM_TOKEN` in env.
- `mastery.py [/mastery]` batched mastery (parallel download, nested-zip walk, one submit).
- `artifacts/<mode>/` start.json + APKs. The mastery/time-trial run writes its start fixture as `wb_start.json` / `wb_mastery_start.json`.

## Results (2026-10-14) — all three modes CORRECT on the first try
- single_solve flag `72f8b691f7ccd6c27823f5716266966c`
- time_trial flag `32ed521e319282d4699fb7c2943f7865` (8 s window, run took ~5 s; 31 attempts left; hall_of_fame_eligible)
- mastery flag `444c5db8366148f36637f3b068b3f05b` (5 sets, bare APKs `cleartext_session_mastery_0N.apk`; 31 attempts left)
- No retries spent beyond the first timed attempt per mode.
- Note `run_for_time_supported` is false on the single_solve route but true for time_trial/mastery routes.
