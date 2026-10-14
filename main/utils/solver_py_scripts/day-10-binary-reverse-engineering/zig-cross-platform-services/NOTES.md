# Zig Zag (day 10, binary-reverse-engineering-day-10-zig-cross-platform-services)

- single_solve CORRECT first try (2026-10-14), flag `54d23a3c00a4c42c8c526fdfd027f56b`.
- Evidence = same d01..d09 bank as day 09 (+d08 67 MB Swift ELF, zip 32 MB) + `d10--variant-000.elf`.
- `solve.py <evidence_dir>` = day-09 solver + d10 block (100 answers).
- d10 = Zig (clang/LLD 19.1.7 zig-bootstrap) x86-64 ELF, stripped, dynamically linked. Plain XOR'd HCFG blob (same
  layout as d04: `HCFG|ver|key|len LE16|crc32 LE|xor json`) in a dedicated `.hcfg` section -> `hcfg()` finds it.
  New field `d10_analysis_has_hcfg_section` = "true"/"false" (section named `.hcfg` present in ELF or PE section table).
  Prompt says ELF *and PE* variants exist; solver handles PE via `section_names()` (untested on a PE sample).
- Deadlines: time_trial 16 s (ranked pool 256), mastery 28 s (3 sets). `live_run.sh`/`mastery_run.sh` copied from day 09.

## Results (2026-10-14)
- single_solve CORRECT, flag `54d23a3c00a4c42c8c526fdfd027f56b`
- time_trial CORRECT first timed attempt (`sh live_run.sh start time_trial`, 10.3 s wall of 16 s deadline, HoF eligible), flag
  `8457d12c56cc119a2494b0df781d9c83`, 255 attempts left.
- mastery CORRECT first timed attempt (`sh mastery_run.sh start`, 21.6 s wall of 28 s deadline, HoF eligible; 300 answers = 3 sets x 100),
  flag `9411dbae171e29dd60fc041eb334f2bb`, 255 attempts left.

## Gotchas
- `run_for_time_supported=false` on this route: it is timed by default. Sending `run_for_time:true` returns
  `{"error":"timed_intent_not_available"}` (no attempt burned). Day-09 scripts hard-coded run_for_time; removed here.
- Mastery: 3 deliveries x 32 MB with a 28 s deadline; sequential download was too risky, so `mastery_run.sh` downloads in
  parallel and `mastery.py` extracts/solves the sets in a thread pool (21.6 s total).
- Mastery sets contained no surprises (d10 ELF in all three seen by solve.py; has_hcfg_section handled). PE d10 path untested.
