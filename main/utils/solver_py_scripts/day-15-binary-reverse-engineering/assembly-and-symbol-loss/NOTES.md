# Day 15 binary RE - Assembly and Symbol Loss ("Stripped")

d15 ships two builds of one ELF: `d15--variant-NNN-symbolized.elf` and `d15--variant-NNN-stripped.elf` (x86-64).
Questions: `d15_artifact_sha256_{1,2}`, `d15_analysis_{stripped,symbolized}_{architecture,format,machine}`,
`d15_analysis_stripped_config_{campaign,domain}` (HCFG as usual).

Graded facts:
- `artifact_sha256_N` follows **zip central-directory order** (symbolized is stored first) - name order was wrong.
  `analyse_set` now groups by CD order (d03 `.dll` then `.runtimeconfig.json` is unchanged).
- `analysis_<tag>_<field>` -> the specimen whose stem ends `-<tag>`; every non-.json member is now fully analysed.
- `machine` with `answer_format.kind == integer` = raw e_machine (`62`); `architecture` = `x86-64`; `format` = `elf`.
- The d14 specimen in this bank is an ELF (was Mach-O on day 14) - the analysis is container-generic.

Cost: mastery = 5 x ~31 MB AES zips; solve took ~13 s on the laptop with all specimens (now 2 per d15) analysed.
Route: dual-pool; TT ranked 64 / regular 32 (80 s); mastery ranked 16 / regular 16 (320 s).
