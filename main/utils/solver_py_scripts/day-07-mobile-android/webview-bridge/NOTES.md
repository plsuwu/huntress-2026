# WebView Bridge (day 07, mobile-android-day-07-webview-bridge)

Same tiny hand-built-APK family as day-04..06 (see their NOTES.md). `assets/huntress_puzzle.json` holds the answer.
Day 7 single_solve codec = `caesar`, key 10: `{"codec":"caesar","data":"KX]7BKPOK?>:CCP=;:@O","key":10}` ->
each byte minus key (mod 256) -> `ANS-8AFEA54099F3106E`. WebView/bridge flavour is decoration; no decompiler needed.
Single flag: `0f711ab9511c894ea8a75f990536eb3d` (CORRECT, first try).

## Layout
- `solve.py <apk>` / `solve.py --submit [/time_trial]`  (copied from day-06; added `caesar` codec + caesar shift in generic BFS;
  start sends `run_for_time:true` when a `/mode` suffix is given). Run with `TMPDIR=$PWD/artifacts/<mode>`.
- `mastery.py [/mastery]` batched mastery (parallel download, nested-zip walk, one submit).
- `artifacts/<mode>/` start.json + APKs.

## Results (2026-10-14) — all three modes CORRECT on the first try
- single_solve flag `0f711ab9511c894ea8a75f990536eb3d`
- time_trial flag `0ff3038665a550f1f94d21304e6efcdd` (window 9 s, run took ~3 s; 15 attempts left; hall_of_fame_eligible)
- mastery flag `1c7d71281af4d050ee5051f142c097f6` (3 sets `set1..3.answer`, bare APKs `webview_bridge_mastery_0N.apk`;
  7 attempts left; hall_of_fame_eligible)
- All 5 APKs seen use `codec: caesar` with an integer `key` (3,4,9,10,18); per-APK key differs, so read it from the JSON.
- The time_trial flag was not printed by `solve.py` (output was truncated); recover with `?op=status&challenge=<id>/time_trial`.
- No retries spent beyond the first timed attempt per mode.
