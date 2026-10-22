# Day 22 binary RE - Managed Native Bridge ("Bridge Troll")

d22 = **three members** (CD order): `d22--sample.dll` (18 KB PE32 i386 .NET assembly, F# on net8.0, metadata
`v4.0.30319`), `d22--sample.runtimeconfig.json` (support) and `d22--FSharp.Core.dll` (5.7 MB PE32+ ReadyToRun,
COFF machine `0xfd1d` = AMD64 ^ the Linux R2R override `0x7b79`; support only). The HCFG record is raw in
`sample.dll` (variant-000: offset 15891, CRC-valid; 2 `HCFG` hits). The existing generic analysis found everything.

New d22 questions (12): `artifact_sha256_{1,2,3}`, `format` (enum -> `pe_cli`), `machine`, `metadata_version`,
`config_{campaign,domain,mutex,url}`, `config_crc32_hex`, `hcfg_version`. All untagged fields say "Use the unique
d22-- primary managed assembly whose basename starts d22-- and ends .dll" - the primary specimen (first non-json
member) is already `sample.dll`, so no routing change was needed. FSharp.Core also starts with `d22--` and ends with
`.dll`; if a future variant puts it first in the CD, route on "primary managed assembly" (skip `FSharp.*` support DLLs).

What broke: only **`d22_analysis_machine`**. The answer `x86` (from `pe::machine_name(0x14c)`) was graded incorrect,
and so was `0x14c`, even though the syntax says "equivalent decimal or hexadecimal code ... accepted". **`i386`** was
graded CORRECT (`file` says "Intel i386"). d03's PE machine question has a different syntax ("PE/COFF ... IMAGE_FILE_MACHINE
labels") and keeps `x86`. Fix in `binary_re.rs::answer_spec`: for a PE with machine 0x14c whose syntax does not name
`IMAGE_FILE_MACHINE`, answer `i386`.

Graded: single CORRECT (first pass incorrect -> grouped oracle isolated `machine` -> one partial submit of `i386` solved
it), TT CORRECT (regular attempt, first submit), mastery CORRECT (6 sets, 1584 answers, first submit). Old days unchanged
(d01-d22 at 12 each = 264).
Route: single run_for_time false; TT dual-pool ranked 64 / regular 32; mastery ranked 16 / regular 16.
Fixtures: `ctf-run/fixtures/binary_re/d22-{single_solve,time_trial,mastery}.json` (replay 57/57 clean). The
single-solve fixture was built by hand: the first-pass answers with `d22_analysis_machine` = `i386` (that run's
`result.json` says incorrect, so `fixtures_from_runs.py` skips it).
