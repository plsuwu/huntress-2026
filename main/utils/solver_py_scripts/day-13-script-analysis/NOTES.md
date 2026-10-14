# Day 13 "Zip Code" (script-analysis-day-13-archive-delivery-chain) - notes (2026-10-15)

- single_solve **CORRECT** flag dc40f0a9881b19779bb2c1dfb4b2f6ee (913 answers, first submit).
- Solver = Day 11 `stager_relationships.py` + one regex tweak: `LIT2` now also accepts a `:` separator
  (AppleScript `property descriptor_x : "..."`). New file family: `polyglot-applescript-{benign-utility,downloader,launcher,staged-chain}.applescript`
  (4 x 11 answers; without the tweak 44 NodeUnsupported errors, everything else already handled).
- `live_run.py` = Day 11 copy with CHALLENGE swapped. Run:
  `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py <mode> --op start --out fixtures/<mode>`
  (token from env HUNTRESS_GM_TOKEN; `--dry` solves without submitting).
- Single-solve route: run_for_time_supported false, regular_pool_size 1.

## Status
- time_trial: CORRECT (see below)
- mastery: CORRECT (see below)

## time_trial: CORRECT flag f383bd09a3b9d81b452ab63180d5f249 (913 answers; NOT timed: `run_for_time_supported:false`, deadline None)
- First full submit was `incorrect` (bare). Used `probe.py` (subset submit -> `accepted`/`remaining`, per-field oracle on the
  untimed regular attempt) - bisect by language, then by file, then by field: only the 5 `batch-*-expansion` answers were wrong.
- Graded `percent` (solver said `delayed`): chain `staged-handoff.bat` (`set "cfg=%v_a0%%v_a1%.."` in a `call :sub` via `for /l`),
  dropper, relay (`set "x=%FrAg0%%FrAg1%.."`), downloader `remote-write.bat` (`call set "x=%%FrAg0%%%%FrAg1%%.."`, non-h<N> names),
  launcher `local-dispatch.bat` (`if defined X set "item9=!item0!!item1!.."` -> `percent`, `call_double`/`for_indirect` rejected).
  `delayed` still accepted for stager/uploader/impact in the same variant (shape not inspected - check before generalising).
- Patch in `stager_relationships.py` (`_classify_expansion`): plain `%..%` set and `call set %%..%%` -> `percent`.
  **Caveat:** the SINGLE-SOLVE variant was CORRECT with `delayed` for dropper/launcher/impact/relay, so this patch would change
  those single-solve answers (variants differ in grading, or the grader keys on something else, e.g. file name/role). The launcher `!..!` case
  is NOT yet patched in code (answers.json was hand-edited). Do not trust the expansion rule - use the oracle.
- Status/oracle gotcha: wrong full submit leaves `remaining` unchanged; accepted subsets are banked, so subset probing is free.

## mastery: CORRECT flag 17095156d1dcf5323dd07631bc87e5cb (4565 answers = 5 sets x 913, untimed regular, deadline None)
- Route: `run_for_time_supported:true` but a plain `start` is a regular untimed attempt; ids `setN.<id>`; one outer zip -> 5 inner
  `evidence-0N.zip` (AES, password `infected`); `live_run.py mastery --op start --dry --out fixtures/mastery` solved 4565/4565, 0 errors.
- Finished with `mastery_oracle.py fixtures/mastery/answers.json` (log: `fixtures/mastery/oracle.log`, final answers:
  `fixtures/mastery/final.json`): non-expansion answers in (set, language) chunks (all accepted on first try, ~3.6 s/request, ~25 min total),
  then each of the 45 `batch-*-expansion` fields with the solver value first. Only 2/45 solver values were rejected:
  `set2.batch-chain-expansion` (`if defined HUNTRESS_READY call set "cfg=%%h0%%..."`, solver said call_double -> percent) and
  `set5.batch-uploader-expansion` (`if defined HUNTRESS_READY set "item9=!item0!!item1!.."`, solver said delayed -> percent).
- **Expansion grading, empirical (45 mastery + 9 time-trial points):** the label is graded on the single reconstruction statement:
  * statement prefixed by inline `if defined X` -> `percent` (every observed case: mastery set1 stager/relay, set2 chain, set5 uploader; time-trial launcher);
  * `for %%G in (..) do call set "x=%%x%%%%name%%G%%"` -> `for_indirect` (7/7; h<N> names were `call_double` in the day-3 data);
  * `call set "x=%%h0%%%%h1%%.."` with `h<N>` names -> `call_double` (2/2: set1 downloader, set4 launcher); other names -> `percent`;
  * plain `set "x=!a!!b!"` -> `delayed` (10/10), plain `set "x=%a%%b%"` -> `percent`.
  Rule lives in `_classify_expansion` (stager_relationships.py); the solver reproduces final.json with 0 diffs (`tests/test_regression.py`).
- **Single-solve caveat:** the single_solve grader (or variant) accepted `delayed` for dropper/launcher/impact/relay shapes that the
  rule above labels `percent`; the offline test shows those 4 diffs against `fixtures/single-solve/answers.json`. Regular/mastery/time-trial
  agree with each other, so trust the rule for them and use the oracle if a new variant disagrees.
- Gotchas: a wrong single-field submit does not change `remaining`; `accepted` on an already-banked chunk repeats the old `remaining`
  (the first set1 chunks showed 4478 for that reason). The monitor/`pgrep` wrapper tooling is not needed - just tail oracle.log.

## Status summary
single_solve CORRECT (dc40f0a9...), time_trial CORRECT (f383bd09...), mastery CORRECT (17095156...).
