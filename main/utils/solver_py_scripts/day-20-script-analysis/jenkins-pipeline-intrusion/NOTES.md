# Day 20 script analysis - Jenkins Pipeline Intrusion

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Cumulative set: 1329 answers (single/TT, +80 vs day 19), 6645 (mastery,
5 sets). Run dirs: `ctf-run/runs-d20/script-analysis-day-20-jenkins-pipeline-intrusion/` (main checkout, first pass).
Fixtures: `ctf-run/fixtures/script/d20-{single_solve,time_trial,mastery}.json`.

New this day (80 answers, more than the usual +55):

- `polyglot-groovy-{benign-utility,downloader,launcher,staged-chain}` (`.groovy`, 4 x 11) and
  `host-jenkins-pipeline-groovy` (11). Plain `final String descriptor_<hex> = "..."` (base64, or hex-of-base64 on the
  host), plus the usual `// HUNTRESS_ANALYSIS_V1:` marker. The first `analyse_literal` pattern matches it.
- `host-classic-asp-vbscript` (`.vbs`, 11, `host_profile=classic_asp`): `descriptor_<hex> = "<base64>"` on its own line
  after `Dim`. It goes to the literal family through the `host-` prefix. Graded `attack=T1059` (bare, taken from the
  descriptor), not `T1059.005`.
- Two more advanced-Bash files, `coprocess-block.sh` (`feature=coprocess_block`, `primitive=coproc_declaration`) and
  `mapfile-process.sh` (`mapfile_process`, `mapfile_process_substitution`), 7 answers each. `analyse_adv_bash` handled
  both unchanged.

Prep hardening (before release, not needed today): the `descriptor_` pattern now accepts escaped quotes
(`descriptor_x = \"...\"`, as in the Day 19 `.aspx` host), so that file no longer depends on the marker. Unit test
`escaped_literal_without_marker` (`testdata/d19-host-aspx-csharp.csx`). Replay 60/60 clean.
