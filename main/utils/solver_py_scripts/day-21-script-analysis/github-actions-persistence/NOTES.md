# Day 21 script analysis - GitHub Actions Persistence

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Cumulative set: 1373 answers (single/TT, +44 vs day 20), 6865 (mastery,
5 sets). Run dirs: `ctf-run/runs-d21/script-analysis-day-21-github-actions-persistence/` (main checkout, first pass).
Fixtures: `ctf-run/fixtures/script/d21-{single_solve,time_trial,mastery}.json`. Replay 63/63 clean.

New this day (44 answers): `polyglot-github_actions-{benign-utility,downloader,launcher,staged-chain}` (`.yml`,
4 x 11, the usual 11 fields). No `host-` file this time.

- The workflow carries the descriptor as an `env:` value, `HUNTRESS_DESCRIPTOR: "<payload>"` (base64, hex-of-base64,
  dotted-fragment hex or plain hex), plus the usual `# HUNTRESS_ANALYSIS_V1:<KIND>-...` marker on line 1.
- The first `analyse_literal` pattern (`descriptor_\w+`) does **not** match `HUNTRESS_DESCRIPTOR:` (no `_` after
  "descriptor"). The answers come from the marker fallback, so this family depends on the marker. Hardening idea
  (not done, the brief said bookkeeping only): also accept `\bdescriptor\s*:\s*"..."` (YAML key), and add a no-marker
  unit test like `literal_without_marker`. Without either, the any-quoted-token fallback should still find it.
