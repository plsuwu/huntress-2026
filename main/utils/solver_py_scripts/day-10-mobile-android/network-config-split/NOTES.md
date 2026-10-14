# Network Config Split (day 10, mobile-android-day-10-network-config-split)

Same tiny hand-built-APK family as day-04..09 (see day-08/09 NOTES.md). `assets/huntress_puzzle.json` holds the answer;
the manifest / network_security_config diff story is decoration — no decompiler needed.
Single-solve codec = `split`: `{"codec":"split","parts":["56C4B5B0D","ANS-4","9D8953"],"order":[2,0,1]}`
(parts[i] goes to position order[i]) -> `ANS-49D895356C4B5B0D`. Existing `split` handler in solve.py covered it.

## Layout
- `solve.py <apk>` / `solve.py --submit [/time_trial]`; run with `TMPDIR=$PWD/artifacts/<mode>`, needs `HUNTRESS_GM_TOKEN`.
- `mastery.py [/mastery]` batched mastery; one submit.
- `artifacts/<mode>/` start fixtures + APKs. (No `unzip` in this env; use python zipfile.)

## Results (2026-10-14)
- single_solve CORRECT, flag `acc87c164eadbb0af1227261e2961f00`
- time_trial CORRECT, flag `b33d9f6f109a6c1e4769aae136890c4c` (13 s window; 15 attempts left; hall_of_fame_eligible)
- mastery CORRECT, flag `130f0b1509c87c08550ce06e3b762be4` (3 sets, 10 s window; 7 attempts left)
- No retries spent.

## Gotcha (differs from day 09)
- `run_for_time_supported` is **false** on this challenge's time_trial/mastery routes (timed by default). Sending
  `"run_for_time": true` returns HTTP 409 `{"error":"timed_intent_not_available"}` (no attempt burned). Plain
  `{"op":"start","challenge":".../time_trial"}` starts the timed attempt. solve.py/mastery.py here omit the flag.
  Check `run_for_time_supported` in `op=list` per route before choosing.
