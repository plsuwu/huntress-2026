# Seven-Zip, No Relation (day 07, metadata-forensics)
Challenge id: `metadata-forensics-day-07-seven-zip-no-relation` (+ `/time_trial`, `/mastery`). Prompt: "count what the
archives claim, distinguish empty space from evidence, find the census that does not add up".

## Status: all three modes CORRECT (0 variants burned; timed attempts remaining 31)
| mode | flag | question(s) seen |
| --- | --- | --- |
| single_solve | 4b849f6124c4dc477b3a9554dbde3902 | `d07-zip-001-zip-eocd-total-entries` -> `2` |
| time_trial (run_for_time) | bb0018f0396353ad74a9c369f5904634 | `d07-zip-002-...` -> `3` |
| mastery (run_for_time) | 7532765085ebf6ddc136b386dc222677 | `set1.d07-zip-003-...` -> `4`, `set2.d07-zip-004-...` -> `5` |

## Observations
- Only kind seen: `zip.eocd_total_entries` for `evidence-drop-NNN.zip`, integer, trim. Answer = the raw "total entries" u16 of
  the EOCD record (`PK\x05\x06`, last occurrence), NOT a count of LFH/CDH. Artifact is a tiny (~400 B) zip inside the download
  (single/time-trial: one inner zip; mastery: outer zip of `metadata-day-07-evidence-0N.zip`, handled by `nested_zip.py`).
- Parse with raw struct, not `zipfile` (the "does not add up" framing hints at EOCD vs actual-entry mismatches in later
  variants; the variants seen had EOCD total == LFH count == CDH count).
- Deadlines: time trial AND mastery 14s (pool 32). Plain `start` is untimed/regular; `--timed` sends `run_for_time:true`.
  Whole loop (start -> download -> submit) ~1s.
- Run: `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} [--timed] [--retry] [--submit]`.
- `HANDLERS` has guessed (unverified) suffixes for other EOCD fields (entries-this-disk, cd-size, cd-offset, comment-length,
  counts). Unknown kinds print `UNHANDLED` + a `FACTS` dump to stderr and block the submit.
