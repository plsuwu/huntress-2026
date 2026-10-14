# Built Different (day 09, metadata-forensics)
Challenge id: `metadata-forensics-day-09-built-different` (+ `/time_trial`, `/mastery`). Prompt: "executables for several
architectures, plus decoys wearing executable extensions. Determine what can actually run where."

## Status
| mode | how | flag |
| --- | --- | --- |
| single_solve | regular | 5249b7330742e89787b8d6f5c1fd20b2 |
| time_trial | regular untimed start (no run_for_time) | bcc5ac303dc9de198b54f5fb527b9e1b |
| mastery | regular untimed start, 5 sets | 69bf938d2eae136cc77717f0df04d95d |
Timed pool 32 (deadline 6 s TT / 7 s mastery); no timed attempt spent (31+ remain; `run_for_time_supported: true`).

## Observations
- Same 12-question bundle as day 08 (see ../../day-08-metadata-forensics/whats-in-the-box/NOTES.md) + one new question:
  `d09-pe-001-pe-coff-machine-u16-hex` for `telemetry-agent-001.exe` -> `0x014c` (i386) in single/TT; format `0x` + 4 lowercase hex.
- `pe_machine()`: MZ -> e_lfanew @0x3C -> `PE\0\0` -> u16 LE; returns literal `none` for decoys (no MZ / no PE sig), as the
  answer-format text allows. `elf_machine()` is a speculative handler for a later variant (not exercised).
- Evidence-note differs per variant ("Case <hex>"); all other media identical to day 08.
- Run: `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} [--timed] [--retry] [--submit]`.
- Unknown question kinds print a FACTS dump and block submit (no burned timed attempt).
