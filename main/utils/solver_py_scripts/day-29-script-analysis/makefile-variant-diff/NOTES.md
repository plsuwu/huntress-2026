# Day 29 script analysis - Makefile Variant Diff

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Answers: 1747 single-solve (+44 vs day 28), 1833 time trial (+52 vs
day 28's 1781), 23829 mastery (**13 sets** of 1833). Run dirs:
`ctf-run/runs-d29/script-analysis-day-29-makefile-variant-diff/` (main checkout, first pass). Fixtures:
`ctf-run/fixtures/script/d29-{single_solve,time_trial,mastery}.json`. Replay 87/87 clean.

New this day: `polyglot-makefile-{benign-utility,downloader,launcher,staged-chain}` (4 x 11, the usual fields).
**No new `host-` file** again (+44). `.mk` files:

- `# HUNTRESS_ANALYSIS_V1:B64-<payload>` marker comment on line 1, then an **unquoted** make variable
  `HUNTRESS_DESCRIPTOR_<hex> := <payload>` and a no-op `.PHONY: analyze-<hex>` target (`@:`). The primary
  `descriptor_\w+ ... "..."` pattern needs quotes, so it misses; the `HUNTRESS_ANALYSIS_V1` marker fallback in
  `analyse_literal` finds it.
- TT/mastery stay on the day-23 schema (hex-id files such as `87bca5.mk`, `<family>-<hex>-<kind>` ids, 13 per
  language family in TT).
