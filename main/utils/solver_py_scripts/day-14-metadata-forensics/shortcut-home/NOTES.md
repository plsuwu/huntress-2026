# Day 14 metadata - The Shortcut Home

One kind: `lnk-target-path-windows` (single-solve: shortcut-001.lnk -> `C:\Windows\System32\cmd.exe`, CORRECT).
The link carries the target twice: LinkInfo LocalBasePath (+ CommonPathSuffix) and the LinkTargetIDList (My Computer
root -> `/C:\` volume item -> file entries whose long names sit in v3 0xBEEF0004 blocks at block offset 20, after a u16 long-string size; names must validate against the short name or the IDList source is dropped). `lnk_target` in
`ctf-run/src/lanes/metadata.rs` answers only when every present source agrees (case-insensitive) and errors on
disagreement - the prompt's "identify the impossible destination" suggests mastery variants where they differ;
if a timed run errors there, inspect the FACTS dump and settle which source is graded via practice.
Time trial (1 set) / mastery (2 sets) are ONE-SHOT (30 s). Fixture `ctf-run/fixtures/metadata/d14-single_solve.json`.

Time trial and mastery (one-shot, 2026-10-15): both CORRECT first time; graded fixtures in `ctf-run/fixtures/<lane>/d14-{time_trial,mastery}.json`, artifacts under `artifacts/`.
