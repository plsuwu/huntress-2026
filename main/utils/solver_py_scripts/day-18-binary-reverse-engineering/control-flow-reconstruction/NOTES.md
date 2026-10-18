# Day 18 binary RE - Control Flow Reconstruction ("Flow State")

d18 = gfortran x86-64 ELF (`MAIN__`), dynamically linked, not stripped, 16.5 KB. Raw HCFG (key 0x31) in `.rodata`
at file offset 8512 (0x2140), validated by CRC. Decoration: functions `event_failure.0` / `event_sandbox.1` /
`event_decoy.2` / `event_success.3` picked via an indirect call (prompt: "reconstruct the call graph ... prove no
injection hides in the flow"), a `HUNTRESS_HOST_MODEL` env-var lookup and a `huntress.binary.dynamic_trace.v1` JSON
template (`family: fortran_indirect_flow_trace`, `effects: modeled_process_access_no_injection`). None of it is asked.

Questions: 216 single-solve (d01-d17 at 12 each, unchanged from Day 17, + 12 for d18): `d18_artifact_sha256_1`,
`d18_analysis_{architecture,format,machine}` (`x86-64`, `elf`, `x86-64`), `d18_analysis_config_{campaign,domain,mutex,url}`,
`config_crc32_hex`, `hcfg_offset`, `hcfg_version`, `xor_key_hex`. No `hcfg_storage` on d18 (if asked later: raw blob
in `.rodata` -> `raw_const_array` by assumption, untested).

**No code change.** Single-solve CORRECT on the automated first pass (first submit).
Time trial / mastery are **timed by default** today (not dual-pool regular attempts), so the first pass skipped them;
they were not run by the lane and are the coordinator's. Expect the same shape as Day 17 (variants incl. stripped d01,
MinGW PE d02 in mastery) - all covered by the d17 fixtures.

Fixture: `ctf-run/fixtures/binary_re/d18-single_solve.json` (replay 43/43 clean).
