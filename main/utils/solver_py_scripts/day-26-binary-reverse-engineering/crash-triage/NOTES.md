# Day 26 binary RE - Crash Triage ("Crash Course")

d26 = **one Groovy JAR** `d26--sample.jar` (4.5 KB, "Huntress deterministic Groovy factory"; Groovy runtime not
bundled): `Main-Class: Day26Variant000`, a `Day26Variant000$_main_closure1` closure class, and a JSON member
`crash/trace.json` = `{"schema":"huntress.synthetic_crash.v1","kind":"IllegalStateException","frames":["stageThree","stageTwo","stageOne"]}`.
The HCFG record is a base64 classfile constant (>= 80 chars), found by `jar_hcfg` as before.

New d26 questions (12): `artifact_sha256_1`, `analysis_format` (enum -> `jar`), `config_{campaign,domain,mutex,url}`,
`config_crc32_hex`, `hcfg_version`, `xor_key_hex`, `method_count` (Main-Class method table: 10), and two new fields:

- `analysis_crash_fault_method`: "Parse crash/trace.json and submit the exact first string in its frames array" -> `stageThree`.
- `analysis_crash_frames_match_methods` (enum true/false): every `frames` string is a method name of the primary
  (Main-Class) classfile -> `true` in every set seen (single, TT, 10 mastery sets). Expect `false` variants later.

Code: `Spec::jar_json` keeps every parsed `.json` member of a jar by path; `trace_frames` takes the jar JSON member the
question text names (exactly one token must match a member path, else Err) and reads its `frames` strings.

All three modes CORRECT on the first submit (untimed): single (312 answers), TT (312), mastery (**10 sets**, 3120).
Old days unchanged (d01-d26 at 12 each = 312).
Fixtures: `ctf-run/fixtures/binary_re/d26-{single_solve,time_trial,mastery}.json` (replay 69/69 clean).
