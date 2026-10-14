# D-Day (day 11, binary-reverse-engineering-day-11-d-collection-planner)

All three modes CORRECT on the first submit (2026-10-14), no oracle probing needed.

| mode | answers | flag |
|---|---|---|
| single_solve | 110 (d01..d11) | `838816c850a9bb4fa081b0c7d3e6e2d9` |
| time_trial (ranked, 30 s, 1 set) | 110 | `f4a7101e247cb1be3268d3b6ec992bc3` |
| mastery (ranked, 30 s, 3 sets `set1.`..`set3.`) | 330 | `57b806bd102590f47acf18e3e78c9ec7` (~26 s wall incl. `start`) |

- Evidence = d01..d10 bank (byte-identical to later days) + **d11 = D-language ELF** (dynamic, not stripped): config is the
  base64 `SENGR...` string constant -> `elf_b64_hcfg`; `d11_analysis_hcfg_offset` = file offset of that base64 text
  (`d.find(b'SENGR')`, e.g. 368272). d11 `format` is `ELF` (uppercase; d12/d13 are lowercase `elf`).
- `solve.py` = day-12 `pascal-archive-stager/solve.py` with the final loop limited to `('d11',)`; `mastery.py`,
  `live_run.sh`, `mastery_run.sh` are day-12 copies with only the challenge id changed.
- Route `run_for_time_supported:false`: timed by default, single ranked attempt (`unique_attempts_remaining` 0 after the
  start); do NOT send `run_for_time`. `hall_of_fame_eligible` false.
- Mastery wall time (~26 s of 30 s) is dominated by the network (start + 3 parallel downloads + 7z extraction); offline
  solve is <1 s. Little slack - don't add work to the one-shot path.
- Usage: `set -a; . ./.env; set +a; sh live_run.sh start time_trial` / `sh mastery_run.sh start`.
  Fixtures: `artifacts/single/`, `artifacts/time_trial/run_*`, `artifacts/mastery/mastery_*`.
