# Day 16 script analysis - Obfuscation Equivalence

Solved natively by `ctf-run` with **no lane change**: single-solve, time trial and mastery (regular, untimed) all
CORRECT on the automated first pass. Cumulative set grew to 1095 answers (single/TT) and 5475 (mastery, 5 sets).
Fixtures: `ctf-run/fixtures/script/d16-{single_solve,time_trial,mastery}.json`; artifacts under `artifacts/` (main checkout).

New this day (69 answers):
- `polyglot-ruby-{benign-utility,downloader,launcher,staged-chain}` (4 x 11) and `host-ruby-rack-ruby` (11,
  `host_profile=ruby_rack`). The literal is a Ruby constant `DESCRIPTOR_<HEX> = "<base64>"` - **uppercase**, so the
  case-sensitive `descriptor_\w+` pattern misses it; `analyse_literal` found it via the `# HUNTRESS_ANALYSIS_V1:B64-...`
  marker fallback (same descriptor). If a future day drops the marker, the quoted-token fallback still applies, but
  making the first pattern case-insensitive would be the robust fix.
- `bash-advanced-ansi-c-quoting-*` and `bash-advanced-regex-conditional-*` (7 each): answered by the existing
  advanced-Bash family.
