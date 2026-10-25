# Day 25 script analysis - Phoenix Stage Chain

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Answers: 1571 single-solve (+55 vs day 24), 1625 time trial (+65),
14625 mastery (**9 sets** now, was 8). Run dirs: `ctf-run/runs-d25/script-analysis-day-25-phoenix-stage-chain/`
(main checkout, first pass). Fixtures: `ctf-run/fixtures/script/d25-{single_solve,time_trial,mastery}.json`.
Replay 75/75 clean.

New this day: `polyglot-elixir-{benign-utility,downloader,launcher,staged-chain}` (4 x 11, the usual fields) plus
`host-elixir-phoenix-elixir` (11 answers, `host_profile=elixir_phoenix`, `attack` graded bare `T1059`). `.exs` files:

- `# HUNTRESS_ANALYSIS_V1:<KIND>-...` marker comment on line 1, the descriptor as a plain Elixir binding
  `descriptor_<hex> = "<payload>"` (then `_plan_<hex> = %{descriptor: descriptor_<hex>}` and an inert
  `defmodule Huntress.InertController<hex>` Phoenix controller). The `descriptor_\w+ = "..."` pattern in
  `analyse_literal` matches it directly (no fallback needed).
- TT/mastery use the day-23 schema unchanged (hex-id files, `LYR-<recipe>-` layered literals); no new question kinds.
