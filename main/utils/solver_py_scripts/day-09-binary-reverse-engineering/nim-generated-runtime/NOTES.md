# Nimble (day 09, binary-reverse-engineering-day-09-nim-generated-runtime)

Status (2026-10-14), static analysis only, nothing executed:
- single_solve CORRECT first try, flag `4b21ac6696b2dd46a441d5c56628e6c4`
- time_trial CORRECT first timed attempt (`live_run.sh start time_trial`, 9.1 s wall of the 10 s deadline; HoF eligible), flag `b4ed321dd9ad53d9ec64710020bceeca`, 31 attempts left
- mastery CORRECT as a **regular untimed** attempt (`mastery_run.sh start`, no run_for_time; 450 answers = 5 sets x 90), flag `2e8f04ce43a768c84993df1bba561dd7`.
  Not HoF eligible. A timed mastery (deadline 20 s, 5 x ~31 MB deliveries ~ 34 s measured download) is not reachable from this link; not attempted.

## Layout
- `solve.py <evidence_dir>`  all 90 answers from evidence bytes -> JSON (= day-08 solve.py + d09 block)
- `live_run.sh [start|retry] [time_trial|mastery]`  one-process start(run_for_time) -> download -> 7z -> solve -> submit
- `artifacts/single_solve/` start.json, evidence.zip, ev/, answers.json, submit_body.json, result.json

## Question shape
90 questions = d01..d08 exactly as day 08 + 10 d09 fields (`artifact_sha256_1`, `architecture`, `format`, `machine`,
`config_{campaign,domain,mutex,url,crc32_hex}`, `hcfg_offset`). d09 = 61 KB Nim ELF x86-64 (not stripped, `-d:release`-ish C backend).
HCFG blob is the same base64 string constant as d08/d06/d07 (`SENGR...`, 1 hit; raw `HCFG` hits are decoys incl. the Nim `raw[0..3] == "HCFG"` assertion string).
Nim string literal layout before the data: `{len(8), cap|0x40<<56 (8)}` then bytes -> the `@` seen in `strings`.
`d09_analysis_hcfg_offset` (integer) = **file offset of the base64 constant's first byte** (30696 = 0x77e8, .rodata vaddr 0x4077e8) - accepted.
Evidence zip is 31 MB (d08 Swift ELF 67 MB dominates), so bandwidth is the limit in time trial (deadline 10 s; download ~5.5 s here).

## Mastery notes
- Deliveries are 5 plain (not nested) AES zips `delivery1..5.zip`, each holding d01..d09 specimens; ids are `setN.<id>`. `mastery.py` is the day-07 one unchanged.
- Variants include a **stripped d01 ELF (no .symtab)**: `solve.py` now answers `d01_analysis_symbols=[]`, `d01_analysis_disassembly=[]`, `has_symtab=false`
  (accepted). Day-08 solve.py crashed on this (StopIteration).
- `d09_analysis_hcfg_offset` was 30696 in all 6 d09 variants seen.
- A regular (untimed) mastery start does not burn timed variants (`regular_variants_remaining` 9 -> 8).
