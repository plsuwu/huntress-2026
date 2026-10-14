# Provider Paths (day 06, mobile-android-day-06-provider-paths)

Status: single_solve, time_trial, mastery all CORRECT (2026-10-14), first try each. Same tiny hand-built-APK family as
day-04 pending-intent-relay / day-05 backup-spill (see their NOTES.md).
Flags: single `a94bd988af673862da8930ee7f39a22e`, time_trial `00e4aa6803dc04727c83373a28f59d6b`, mastery `b935e6a264d365489fe8f344184cb4d7`.
Pools after use: time_trial 15 attempts left, mastery 7 left; both `hall_of_fame_eligible: true`.

## Layout
- `solve.py <apk>`  print answer; `solve.py --submit [/time_trial]` start -> download -> solve -> submit in one process
  (timed window ~6 s, run took ~3 s). Saves `pp_start.json` + `pp.apk` to `$TMPDIR` (run with `TMPDIR=$PWD/artifacts/time_trial`).
- `mastery.py [/mastery]`  start -> parallel download of `artifacts[]` APKs -> decode each -> one batched submit
  (3 sets, ids `set1..3.answer`). Copied from day-05 backup-spill unchanged apart from names.
- Token: `set -a; . ../../../../.env; set +a` (path from this dir; or run from repo root).
- `artifacts/{single_solve,time_trial,mastery}/` start.json + APKs (single_solve/files = `7z x` unpack).

## The puzzle
One question `answer`, format `ANS-` + 16 uppercase hex. `assets/huntress_puzzle.json` holds it in plain sight:
`{"schema":"huntress.mobile.puzzle.v1","day":6,"codec":"hex","data":"414e532d..."}` -> `bytes.fromhex(data)` = `ANS-46A24BCDAE5C6E1B`.
All 5 APKs seen (single, time_trial, 3 mastery) use codec `hex` with no key. Day 6 flavour (case MOB-06-..., package
`com.huntress.mobile.d06.v<hex>`, exported ContentProvider `ProviderPathsProvider`, WebView to unreachable loopback evidence URL)
is decoration - the "overbroad provider path" exploit is never needed. `7z x` + `json` suffices; no decompiler.

## Hardening
- `decode` handles codecs hex, base64, xor(+key), split(parts/order); unknown codec -> depth-3 BFS over reverse/hex/b64/b32/rot13/xor-key
  (tested offline on reversed-b64, reversed-hex, b32+xor), then NotImplementedError with the puzzle JSON.
- Because timed windows are 6 s, keep it a single process; on a failed decode the script does not submit and the fixture is on disk.
