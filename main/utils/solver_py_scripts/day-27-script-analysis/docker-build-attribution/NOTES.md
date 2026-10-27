# Day 27 script analysis - Docker Build Attribution

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Answers: 1659 single-solve (+44 vs day 26), 1659 time trial, 18249
mastery (**11 sets** of 1659). Run dirs: `ctf-run/runs-d27/script-analysis-day-27-docker-build-attribution/`
(main checkout, first pass). Fixtures: `ctf-run/fixtures/script/d27-{single_solve,time_trial,mastery}.json`.
Replay 81/81 clean.

New this day: `polyglot-dockerfile-{benign-utility,downloader,launcher,staged-chain}` (4 x 11, the usual fields).
**No new `host-` file** again (+44). `.dockerfile` files:

- `# HUNTRESS_ANALYSIS_V1:HEX-<hex>` marker comment on line 1, then `FROM scratch AS analysis_<hex>`,
  `ARG HUNTRESS_DESCRIPTOR="<hex>"` and `LABEL huntress.analysis.descriptor=$HUNTRESS_DESCRIPTOR`. There is no
  `descriptor_<x> = ...` binding (`HUNTRESS_DESCRIPTOR=` has no `_` suffix after `DESCRIPTOR`), so `analyse_literal`
  finds it via the `HUNTRESS_ANALYSIS_V1` marker fallback (the quoted-literal fallback would also match).
- **TT/mastery changed schema**: they no longer use the day-23 hex-id/`LYR-` schema. The time trial's question ids
  and named files are identical to single-solve's (1659 = 1659, d26 TT was 1677 hex-id answers), and each mastery
  set is the same named-file layout (`setN.<single id>`, `evidence-NN.zip -> <name>`). The existing code handled it.
