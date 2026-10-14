# Pascal's Wager (day 12, binary-reverse-engineering-day-12-pascal-archive-stager)

All three modes CORRECT on the first submit (2026-10-14), no oracle probing needed.

| mode | answers | flag |
|---|---|---|
| single_solve | 120 (d01..d12) | `05052974dd33c6dda9d527cffede4fe9` |
| time_trial (ranked, 30 s, 1 set) | 120 | `37020ccf1f6d9e16c3c178eaf3b71bde` (7 s wall) |
| mastery (ranked, 30 s, 3 sets `set1.`..`set3.`) | 360 | `4da37b4b11e1087cecb94c3923b9107a` (~17 s wall) |

- Evidence = the day-13 bank minus d13: d01..d11 byte-identical to earlier days + **d12 = Pascal ELF** (static, stripped,
  plain XOR'd HCFG blob; `d12_analysis_hcfg_offset` = offset of the CRC-validated `HCFG` magic, e.g. 82928).
- `solve.py` = day-13 `crystal-dns-configuration/solve.py` with the final loop limited to `('d11', 'd12')` (d13 file is
  absent -> `pick('d13')` would crash). `mastery.py` is unchanged (7z-extracts each delivery, solves the dir holding `d01--*`).
- d12 `format` must be lowercase `elf` (same casing rule learned on day 13; d11 is `ELF`).
- Route `run_for_time_supported:false`: single is untimed; time_trial/mastery are timed by default with a **single ranked
  attempt** (`unique_attempts_remaining` 0 after the first start). Do NOT send `run_for_time` (`timed_intent_not_available`).
  `live_run.sh` / `mastery_run.sh` therefore do not send it (mastery_run.sh only does if given a 2nd arg - don't).
- Offline solve takes ~0.4 s for 3 sets; the 30 s budget is dominated by the network (start + 3 parallel downloads).
- Usage: `set -a; . ./.env; set +a; sh live_run.sh start time_trial` / `sh mastery_run.sh start`.
  Fixtures: `artifacts/single/` (start/evidence/answers/result), `artifacts/time_trial/run_*`, `artifacts/mastery/mastery_*`.
- Tooling hazard: in this harness, long multi-part shell calls with heredocs sometimes hang; redirect stdin from /dev/null
  and prefer the Write tool for notes.
