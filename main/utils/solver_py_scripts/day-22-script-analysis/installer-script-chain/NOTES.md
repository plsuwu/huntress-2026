# Day 22 script analysis - Installer Script Chain

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Cumulative set: 1417 answers (single/TT, +44 vs day 21), 8502 (mastery,
**6 sets** now, was 5). Run dirs: `ctf-run/runs-d22/script-analysis-day-22-installer-script-chain/` (main checkout,
first pass). Fixtures: `ctf-run/fixtures/script/d22-{single_solve,time_trial,mastery}.json`. Replay 66/66 clean.

New this day (44 answers): `polyglot-nsis-{benign-utility,downloader,launcher,staged-chain}` (`.nsi` NSIS installer
scripts, 4 x 11, the usual 11 fields). No `host-` file.

- The descriptor is an NSIS register assignment inside a `Section`: `StrCpy $0 "<payload>"` (no `descriptor_`
  name), encodings base64, hex, hex-of-base64 and dotted-fragment hex, plus the usual
  `; HUNTRESS_ANALYSIS_V1:<KIND>-...` marker on line 1.
- `analyse_literal`'s `descriptor_` pattern does not match; answers come from the marker fallback. Without the
  marker the any-quoted-token fallback would still find it (the `StrCpy` string is the only long quoted token;
  `Name`/`OutFile` strings contain spaces/dashes and are short).
