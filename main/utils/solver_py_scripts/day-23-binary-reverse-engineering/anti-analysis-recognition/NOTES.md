# Day 23 binary RE - Anti-Analysis Recognition ("Trust Issues")

d23 = **two members** (CD order): `d23--sample.dll` (8 KB PE32 i386 .NET assembly, VB.NET on net8.0 via
`Microsoft.VisualBasic.Core`, metadata `v4.0.30319`) and `d23--sample.runtimeconfig.json` (support). The gimmick is
anti-analysis predicates (`Debugger.IsAttached`, a `Stopwatch` timing check, the `HUNTRESS_HOST_MODEL` env var with
`workstation`/`analysis`/`sandbox` values) choosing `decoy`/`success`/`failure` branches of an inert JSON trace
(`huntress.binary.dynamic_trace.v1`, family `vbnet_anti_analysis_predicate_trace`). None of it is scored: the HCFG
record is raw in the DLL (variant-000: one `HCFG` hit at offset 7680, CRC-valid).

New d23 questions (12): `artifact_sha256_{1,2}`, `format` (enum -> `pe_cli`), `machine` (`i386`, the d22 rule),
`metadata_version`, `config_{campaign,domain,mutex,url}`, `config_crc32_hex`, `hcfg_version`, `xor_key_hex`. Same
"Use the unique d23-- primary managed assembly ... ends .dll" wording as d22; the primary (first non-json member) is
the DLL, so no routing change.

**No code change.** The automated first pass got all three modes CORRECT on the first submit (untimed): single (276
answers), TT (276), mastery (7 sets, 1932 answers). Old days unchanged (d01-d23 at 12 each = 276).
Route: single run_for_time false; TT dual-pool ranked 64 / regular 32 (80 s); mastery ranked 16 / regular 16 (465 s).
Fixtures: `ctf-run/fixtures/binary_re/d23-{single_solve,time_trial,mastery}.json` (replay 60/60 clean).
