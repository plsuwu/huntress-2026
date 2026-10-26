# Day 26 script analysis - BEAM Message Flow

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Answers: 1615 single-solve (+44 vs day 25), 1677 time trial (+52),
16770 mastery (**10 sets** now, was 9). Run dirs: `ctf-run/runs-d26/script-analysis-day-26-beam-message-flow/`
(main checkout, first pass). Fixtures: `ctf-run/fixtures/script/d26-{single_solve,time_trial,mastery}.json`.
Replay 78/78 clean.

New this day: `polyglot-erlang-{benign-utility,downloader,launcher,staged-chain}` (4 x 11, the usual fields).
**No new `host-` file** this time (hence +44, not +55). `.escript` files:

- `% HUNTRESS_ANALYSIS_V1:<KIND>-...` marker comment on line 1, then `-module(plan_<hex>).` and the descriptor as
  the return value of a function, `build_plan() -> "<payload>".` - there is no `descriptor_<x> = ...` binding, so
  `analyse_literal` finds it via the `HUNTRESS_ANALYSIS_V1` marker fallback (the quoted-literal fallback would also
  match).
- TT/mastery use the day-23 schema unchanged (hex-id files, `LYR-<recipe>-` layered literals); no new question kinds.
