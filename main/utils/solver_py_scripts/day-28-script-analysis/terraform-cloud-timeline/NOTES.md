# Day 28 script analysis - Terraform Cloud Timeline

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Answers: 1703 single-solve (+44 vs day 27), 1781 time trial (+122 vs
day 27's 1659), 21372 mastery (**12 sets** of 1781). Run dirs:
`ctf-run/runs-d28/script-analysis-day-28-terraform-cloud-timeline/` (main checkout, first pass). Fixtures:
`ctf-run/fixtures/script/d28-{single_solve,time_trial,mastery}.json`. Replay 84/84 clean.

New this day: `polyglot-terraform_hcl-{benign-utility,downloader,launcher,staged-chain}` (4 x 11, the usual fields;
note the `_` inside the language token). **No new `host-` file** again (+44). `.tf` files:

- `# HUNTRESS_ANALYSIS_V1:B64-<payload>` marker comment on line 1, then `terraform { ... }`,
  `locals { huntress_descriptor_<hex> = "<payload>" }` and an `output` referencing it. The `locals` binding matches
  the primary `descriptor_\w+ = "..."` pattern in `analyse_literal` (the `huntress_` prefix is irrelevant), so the
  marker fallback is not even needed.
- **TT/mastery are back on the day-23 schema** (hex-id files such as `5448a4.tf`, `<family>-<hex>-<kind>` ids),
  after day 27 had used single-solve's named layout. Both schemas are handled, so
  expect either on future days.
