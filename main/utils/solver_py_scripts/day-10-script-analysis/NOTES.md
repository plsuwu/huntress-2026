# Day 10 "Bind Over Matter" (script-analysis-day-10-wmi-persistence-reconstruction) - notes (2026-10-15)

All three modes **CORRECT** on first submit, no solver errors:
- single_solve f4e9898b706e93fcc804b32720e37f51 (753 answers)
- time_trial   b0e906d2a5424e1602651d0389712c77 (753 answers; start 4.8 s, submit ~3 s; deadline 13 s; hall_of_fame_eligible true; unique_attempts_remaining 31)
- mastery      2176b9f11980dbe2090879879cc36d1c (1506 = 2 x 753; deadline 32 s; unique_attempts_remaining 31)

## What was new vs Day 9 (695 answers -> 753)
- `polyglot-wmi_mof-{benign-utility,downloader,launcher,staged-chain}` (4 x 11/10): `.mof` files. Descriptor is the
  `Descriptor = "..."` string of the `instance of HuntressAnalysis_xxx { ... }` (the `// HUNTRESS_ANALYSIS_V1:<ENC>-...`
  header comment carries the same payload). Encodings seen: FRAG (dotted hex fragments), HEX, B64, HB64 (hex of base64) -
  all handled by the existing `decode_descriptor` (strip `.`, loop hex/base64 until `v=`).
- `bash-advanced-{here-string,heredoc-reconstruction}` (2 x 7): already handled by the Day 7/8 advanced-bash analyser.
- Only code change vs Day 9 `stager_relationships.py`: added `MOF_LIT` regex fallback next to `LIT2` in `solve()`.
  The prompt text (persistence subscriptions/bindings) is flavour; the questions are the usual descriptor-field ones
  and values are submitted verbatim.

## Gotchas
- `run_for_time_supported: false` for TT/mastery on this route => plain `start` is **timed by default** (13 s / 32 s);
  there is no regular untimed pool (regular_pool_size 0). No dry run is possible before the timer begins - the solver
  was validated on the single-solve fixture instead (same question schema; mastery = single x set1./set2.).
  Client latency (~5 s start, 3-4 s submit) fits but is not generous; ranked pool 32 so retries are available.
- Run: `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py <mode> --op start --out DIR`
  (token from env `HUNTRESS_GM_TOKEN`).
- Fixtures: `fixtures/{single-solve,time-trial,mastery}` (start.json, downloads, answers.json, result.json).
- Not done: Rust port, optimisation of timed latency for Hall of Fame.
