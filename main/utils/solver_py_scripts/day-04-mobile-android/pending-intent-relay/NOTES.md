# Pending Intent Relay (day 4, mobile-android) - single_solve

Status: single_solve CORRECT; time_trial CORRECT (hall_of_fame_eligible, 15 attempts left). mastery CORRECT (hall_of_fame_eligible, 6 attempts left).

- ID: `mobile-android-day-04-pending-intent-relay` (+ `/time_trial`, `/mastery`; ranked_deadline_ms=6000 -> expect script-only speed)
- One question, id `answer`, format `ANS-` + 16 uppercase hex. Artifact: `pending_intent_relay.apk` (~7 KB, tiny hand-built APK).
- Download via `?op=download&capability=...` from the `start` response (needs bearer header).

## What's in the APK
- `assets/huntress_puzzle.json`: `{"schema":"huntress.mobile.puzzle.v1","codec":"xor","data":<b64>,"key":29}`
- `classes.dex` (2.6 KB): 3 classes (MainActivity, ChallengeActivity, PendingIntentRelay); strings include a
  loopback evidence URL `http://127.0.0.1:19090/mobile/attempt/<id>/evidence?case_id=` + `client_secret=SECRET-...&proof=`
  - theme-only decoration; backend isn't reachable and not needed.
- Manifest: deep link `huntress04://pending-intent-relay/unlock`, meta-data `huntress.mechanic = mutable PendingIntent relay`.

## Solution
base64-decode `data`, XOR every byte with `key` -> `ANS-...` directly. The "mutable PendingIntent" narrative is flavour;
the answer is just the puzzle asset. `solve.py` derives it from the APK (no hardcoded values) and validates the pattern.

## Notes for time_trial / mastery
- Variants may change `codec`/`key` (solve.py raises NotImplementedError for non-xor so it fails loudly) and, in mastery,
  the APKs probably arrive inside nested zips (`utils/solver_py_scripts/nested_zip.py`).
- Tooling: no jadx/apktool/aapt in env; `7z x` unpacks the APK, `strings`/regex over the dex was enough.

## time_trial (done)
- `python3 -I solve.py --submit /time_trial` runs start -> download -> solve -> submit in one shot; same xor puzzle asset
  (artifact `pending_intent_relay_time_trial.apk`). Accepted at 12:11:58.976Z with deadline 12:12:02.263Z (~3.3 s margin;
  deadline is ~6 s after start). Flag: 0675cfb2bed051596e61f9fd4d3bb705.
- Keep it a single script: separate manual steps would blow the 6 s window.

## mastery (done)
- 3 question sets (`set1.answer`..`set3.answer`), each its own bare APK (NOT zipped/nested): the start response has
  `artifacts[] = {download, filename, sha256}` (and `downloads[]`) instead of a single `download`. Same xor puzzle in each.
- `mastery.py [/mastery]` handles it: start -> parallel download -> decode each -> single batched submit (~4 s after start,
  deadline is start+6 s). It also copes with a single `download` (time_trial) and zips containing APKs.
- Gotcha: my first run crashed on `KeyError: 'download'` (mastery has no top-level `download`); that expired attempt
  cost one of the 8 variants, then `op=retry` -> run script (start reconnects to the retried attempt). Flag: 07357fcc1db236feadd46fb04e65e9c0.
- Lesson: a timed start can't be previewed; make the script tolerant of response shape BEFORE the first start.
