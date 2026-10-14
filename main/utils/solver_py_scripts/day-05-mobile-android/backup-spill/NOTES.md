# Backup Spill (day 05, mobile-android-day-05-backup-spill)

Status: single_solve, time_trial, mastery all CORRECT (2026-10-14). Same tiny hand-built-APK family as day-04 pending-intent-relay
(see `../../day-04-mobile-android/pending-intent-relay/NOTES.md`).
Flags: single `a6cc72caf40f41971cc7be1d55ad2d89`, time_trial `e5a9638600d87aae19ba266d9520b9d8`, mastery `92dd8a9e79ae33a67a06895da7434e89`.

## Layout
- `solve.py <apk>`  print answer; `solve.py --submit [/time_trial]` start -> download -> solve -> submit in one process (timed deadline = start+6 s).
- `mastery.py [/mastery]`  start -> parallel download of every `artifacts[]` APK -> decode each -> one batched submit (3 sets, ids `set1..3.answer`;
  also works for single/time_trial). Dumps raw start + APKs to `$TMPDIR` (run with `TMPDIR=$PWD/artifacts/mastery` to keep them).
- Token: `set -a; . ../../../../.env; set +a`.
- `artifacts/single_solve/` start.json, APK, unpacked `files/`; `artifacts/time_trial/bs.apk`; `artifacts/mastery/` start.json + 3 APKs.

## The puzzle
One question `answer`, format `ANS-` + 16 uppercase hex. APK `assets/huntress_puzzle.json` carries the whole answer:
`{"codec":"split","data":"","parts":["0A353BE7B","ANS-5","709E18"],"order":[2,0,1]}`
`parts[i]` goes to position `order[i]` -> `ANS-5` + `709E18` + `0A353BE7B` = `ANS-5709E180A353BE7B` (16 hex chars confirms the orientation;
the other reading, join parts in `order` sequence, gives garbage). Same permutation [2,0,1] in all 5 APKs seen.
The dex/manifest only add flavour (package `com.huntress.mobile.d05.v<hex>`, case id MOB-05-..., WebView -> loopback evidence URL that is
unreachable and not needed). No decompiler is required; `7z x` + `json` is enough.

## Variants / hardening
- day 4 used codec `xor`; day 5 `split`. `solve.decode` handles xor, split, and a generic fallback (base64/hex/b32/reversed/rot13/xor-with-key
  of `data` or joined `parts`, regex for ANS-hex). Unknown shape raises NotImplementedError with the puzzle JSON for inspection.
- Timed attempts: both modes succeeded first try within the 6-7 s window (~3 s). Pools: time_trial 16, mastery 8 (one used).
- Mastery `hall_of_fame_eligible: true` here (unlike the binary lane).
