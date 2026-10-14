# Local Secrets (day 11, mobile-android-day-11-local-secrets)

Same tiny hand-built-APK family as day-04..10 (see day-10 NOTES.md). `assets/huntress_puzzle.json` holds the answer;
the "active credentials vs decoys / local stores" story is decoration — no decompiler needed.
Codec in all modes = `xor`: `{"codec":"xor","data":"<b64>","key":<int>}` -> `ANS-<16 hex>`. The existing xor handler
(copied from day-10 `solve.py`/`mastery.py`, only `CHALLENGE` changed) covered every variant.

## Layout
- `solve.py <apk>` / `solve.py --submit [/time_trial]`; run with `TMPDIR=$PWD/artifacts/<mode>`, needs `HUNTRESS_GM_TOKEN`.
- `mastery.py [/mastery]` batched mastery (3 bare APKs `local_secrets_mastery_0N.apk`, one submit).
- `artifacts/<mode>/` start fixtures + APKs.

## Results (2026-10-14) — all three modes CORRECT on the first try
- single_solve flag `f84f7caa159e540218f88dbe0bc06aa2`
- time_trial flag `0770fb9d14c0277966b134847800554f` (7 s window; 15 attempts left; hall_of_fame_eligible)
- mastery flag `0afc2550b9c27f4fd66361b501d40fb4` (3 sets, 8 s window; 7 attempts left; hall_of_fame_eligible)
- No retries spent. `run_for_time_supported` false on all routes (timed by default; don't send run_for_time).
