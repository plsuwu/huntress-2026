# Day 12 "AutoIt Takes Two" (script-analysis-day-12-autoit-workflow-pair) - notes

Result: all three modes CORRECT on the first submit, no oracle needed.

| mode | answers | flag |
|---|---|---|
| single_solve | 855 | bc2579c6e80c0fec501843869e6b9c54 |
| time_trial (ranked, 30 s) | 855 | 9a0c598925eb305a7cd06f26e6dc6f3a |
| mastery (ranked, 30 s, 2 sets `set1.`/`set2.`) | 1710 | 591e26bc623fedd4f4ce10724028e8a9 |

- Solver = the day-13 `stager_relationships.py` **unchanged** (it already handles the AutoIt `polyglot-autoit-{benign-utility,
  downloader,launcher,staged-chain}.au3` family: 4 files x 11 answers = 44 au3 questions); `live_run.py` = day-13 copy with
  CHALLENGE swapped. Run (token from env; `--dry` solves without submitting):
  `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py <single_solve|time_trial|mastery> --op start [--dry] --out fixtures/<mode>`
- Bundle is cumulative: ps1 (resource/collection/impact), js/cmd/bat/vbs workflow families (8 q each), py recovered
  variants, sh/php/mjs/lua/mof/ts/au3 polyglots. `evidence.zip` is AES (`infected`) -> pyzipper.
- Route `run_for_time_supported:false`: single is untimed; time_trial/mastery are timed by default with a single ranked
  attempt (`unique_attempts_remaining` 0 after the start). Whole start->download->solve->submit took ~10-11 s of the 30 s
  deadline (about 4.4 s of that is the `start` call itself for the big question JSON), so no per-field slack.
- The prompt's "which pairing is genuine" hint did not need special logic: au3 `stage-profile` / `next-language` answers are
  derived by the existing relationship code.
- Fixtures: `fixtures/{single-solve,time-trial,mastery}/` (start.json, delivery zip, answers.json, result.json).
- Hazard (tooling): large mixed bash calls with heredocs + `7z` occasionally hung the harness shell; pass `</dev/null` and
  avoid `7z x` without `-y`-less prompts - live_run.py does the extraction itself via pyzipper.
