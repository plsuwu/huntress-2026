# SQLite Assumptions (day 12, mobile-android-day-12-sqlite-assumptions)

Same tiny hand-built-APK family as day-04..13 (see day-13 `keystore-misuse/NOTES.md`). `assets/huntress_puzzle.json`
holds the answer; the "SQLite cache / server-side impact" story is decoration - no decompiler or SQLite parsing needed.
Single-solve codec = `hex`: `{"schema":"huntress.mobile.puzzle.v1","day":12,"codec":"hex","data":"<hex of ANS-...>"}`
(handled by the existing `decode()`). `solve.py` / `mastery.py` are day-13 copies with only `CHALLENGE` swapped.

## Results (2026-10-14) - all three modes CORRECT on the first try
| mode | flag |
|---|---|
| single_solve | `ddf9b6e54287084fc5423fec59705e9e` |
| time_trial (ranked, 30 s, 1 question) | `c1b59773d78774094660079c55667a0b` |
| mastery (ranked, 30 s, 3 questions) | `afa1469372b6810bb859f7b4697a4c99` |

## Route notes
- `run_for_time_supported:false`: time_trial / mastery are timed by default with one ranked attempt
  (`unique_attempts_remaining` 0 after the start); do not send `run_for_time`. The start->download->decode->submit round trip
  took ~1-2 s of the 30 s window. `hall_of_fame_eligible` was false (pool of 1 / route not HoF-enabled).
- Mastery: 3 bare APKs, one question each; `mastery.py` maps question -> APK by artifact name, one batched submit.

## Usage
`set -a; . ./.env; set +a; TMPDIR=$PWD/artifacts/<mode> python3 -I solve.py --submit` (single) /
`python3 -I mastery.py /time_trial|/mastery`. Fixtures (start JSONs + APKs) in `artifacts/<mode>/`.
