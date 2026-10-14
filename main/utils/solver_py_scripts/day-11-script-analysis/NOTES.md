# Day 11 "By Any Other Name" (script-analysis-day-11-native-api-semantics) - notes (2026-10-15)

- single_solve **CORRECT** flag 07278172f27dc43f2e3d37b22394e61a (811 answers, first submit).
- Solver = Day 10 `stager_relationships.py` + one regex tweak: `LIT2` now accepts a TS type annotation
  (`const descriptor_x: string = "..."`). New file type: `polyglot-typescript-{benign-utility,downloader,launcher,staged-chain}.ts`
  (4 x 11 answers). Without the tweak: 44 NodeUnsupported errors, everything else already solved.
- New extra file families already handled by existing code: `resource/collection/impact-sample.ps1`
  (scenario/attack/url/wallet/mutex etc.), host-*, zipapp/wheel/pyNexe python, polyglot php/lua/mof.
- Run: `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py <mode> --op start --out fixtures/<mode>`
- Gotcha: extracting evidence.zip with pyzipper directly hung the shell (>120 s); use the solver's `ep.read_zip` instead.
- Gotcha: `head` on the start curl truncates the saved file (SIGPIPE) - redirect to file first.

## All modes CORRECT (first submit each)
- single_solve 07278172f27dc43f2e3d37b22394e61a (811 answers)
- time_trial   04692253edf8d15aa9385a67337e350d (811; deadline 9 s; start 4.6 s, submit ~4.3 s; hall_of_fame_eligible true; unique_attempts_remaining 31)
- mastery      86ee07dda07a330b8def981942753f25 (1622 = 2 x 811; deadline 11 s; unique_attempts_remaining 31)
- TT/mastery routes have `run_for_time_supported:false` => plain `start` is timed by default; no regular pool, no dry run.
  Latency is the main risk (client round trips ~4-5 s each for start/submit vs 9 s / 11 s deadlines); still passed.
- Fixtures: `fixtures/{single-solve,time-trial,mastery}` (start.json, download, answers.json, result.json).
- Not done: Rust port; latency optimisation for Hall of Fame.
