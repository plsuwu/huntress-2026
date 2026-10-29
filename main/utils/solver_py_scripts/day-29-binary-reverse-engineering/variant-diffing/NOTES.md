# Day 29 binary RE - Variant Diffing ("Fraternal Twins")

d29 = **two x86-64 ELF builds of one source** (CD order): `d29--sample-O0.elf`, `d29--sample-O3.elf`, raw HCFG
(found by `hcfg::raw`, CRC-validated), one shared config (`BR29-000`). The optimization differences themselves are
not scored.

New d29 questions (14): `artifact_sha256_{1,2}` (CD order), `analysis_{optimized,unoptimized}_{format,machine}`
(enum `elf`, integer `62`), untagged `config_{campaign,domain,mutex,url}` ("Decode both d29-- role members and require
their config objects to be byte-for-byte equal") and `analysis_optimized_config_{campaign,domain,mutex,url}`.
Old days unchanged (406 answers = d28's 392 + 14). Wording now says "Scan backward from the end of the member for
literal HCFG magic and accept the first CRC32-valid record" but also "the unique CRC32-valid record": the forward
`hcfg::raw` hit was graded correct.

**What broke:** the role tags (`optimized` / `unoptimized`) are *not* in the member names (`-O3` / `-O0`); only the
question text maps them ("Use the unique d29-- member whose basename ends O3.elf"). Fix in `lanes/binary_re.rs`: when
no name tag matches and the question names a member, answer from that member, dropping leading words of the field
(`optimized_config_campaign` -> `config_campaign`) while the field is unknown.

All three modes CORRECT untimed on the first submit after the fix: single (406), TT (406), mastery (**13 sets**,
5278). No ranked attempts spent.
Fixtures: `ctf-run/fixtures/binary_re/d29-{single_solve,time_trial,mastery}.json` (replay 78/78 clean).

Day 30 prediction: another pair/trio of builds whose role names come only from the question text (stripped vs
symbolized, PIE vs static, debug vs release), possibly with a role-tagged field other than config/format/machine.
