# Trust Everyone (day 09, mobile-android-day-09-trust-everyone)

Same tiny hand-built-APK family as day-04..08 (see day-08 NOTES.md). `assets/huntress_puzzle.json` holds the answer;
the "challenge CA / permissive trust" story is decoration — no network, no decompiler needed.
Single-solve codec = `base64`: `{"codec":"base64","data":"QU5TLT..."}` -> `ANS-3D3B80395240B8EE`.
`solve.py` / `mastery.py` are copies of day-08 with only `CHALLENGE` changed; the existing base64 codec handled it.

## Layout
- `solve.py <apk>` / `solve.py --submit [/time_trial]`; run with `TMPDIR=$PWD/artifacts/<mode>`, needs `HUNTRESS_GM_TOKEN`.
- `mastery.py [/mastery]` batched mastery (parallel download, 5 bare APKs `trust_everyone_mastery_0N.apk`, one submit).
- `artifacts/<mode>/` start fixtures + APKs.

## Results (2026-10-14) — all three modes CORRECT on the first try
- single_solve flag `254d040e6fb09420d08208f2c7431990`
- time_trial flag `309ef6c731823ebb672f4dcc24e968e3` (8 s window; 31 attempts left; hall_of_fame_eligible)
- mastery flag `def92f609eaf49d4666ba23e65de47bd` (5 sets, 11 s window; 31 attempts left)
- No retries spent. `run_for_time_supported` is false on single_solve route, true on time_trial/mastery.
