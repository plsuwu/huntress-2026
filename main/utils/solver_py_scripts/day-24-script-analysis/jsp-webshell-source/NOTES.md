# Day 24 script analysis - JSP Webshell Source

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Answers: 1516 single-solve (+55 vs day 23), 1560 time trial (+65),
12480 mastery (**8 sets** now, was 7). Run dirs: `ctf-run/runs-d24/script-analysis-day-24-jsp-webshell-source/`
(main checkout, first pass). Fixtures: `ctf-run/fixtures/script/d24-{single_solve,time_trial,mastery}.json`.
Replay 72/72 clean.

New this day: `polyglot-jsp-{benign-utility,downloader,launcher,staged-chain}` (4 x 11, the usual fields) plus
`host-jsp-webshell-jsp` (11 answers, `host_profile=jsp_webshell`). `.jsp` files:

- `<%-- HUNTRESS_ANALYSIS_V1:<KIND>-... --%>` marker on line 1, and the descriptor as a JSP declaration
  `<%! private static final String DESCRIPTOR_<HEX> = "<payload>"; %>`. The case-insensitive `descriptor_\w+ = "..."`
  pattern in `analyse_literal` matches it directly (no fallback needed).
- TT/mastery use the day-23 schema unchanged (hex-id files, `LYR-<recipe>-` layered literals such as
  `b64_frag_b32`, `hex_b64_hex`, `b32_hex_frag`); no new question kinds in time trial.
