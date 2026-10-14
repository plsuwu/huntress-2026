# Day 9 "All the World's a Stager" (script-analysis-day-09-stager-relationships) - notes (2026-10-15)

Single solve **CORRECT** on 1st submit (flag f843b1ccdf78b62fb409b322d08007df). 695 answers = Day 8's 615 (unchanged,
`../day-08-script-analysis/lost_in_translation.py`, copied here as `stager_relationships.py`) + 80 new:
- `polyglot-lua-{benign-utility,downloader,launcher,staged-chain}` (4 x 11), same descriptor anatomy as PHP polyglots;
- `host-sct-scriptlet-jscript`, `host-lua-openresty-lua` (2 x 11): only needed `host-sct-`/`host-lua-` added to `ML_PREFIXES`;
- `bash-advanced-{trap-handler,recursive-walk}` (2 x 7): the Day 8 advanced-Bash analyser already handled them.
Only code change vs Day 8: ML_PREFIXES. Descriptor values are submitted verbatim (even when role/attack look odd,
e.g. lua benign_utility with steps acquire_remote,dispatch_process and attack none) - accepted again.
- Run: `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py <mode> --op start [--out DIR]`
  (single solve: `stager_relationships.py --dir fixtures/single-solve/ex --questions .../start.json`).
- single_solve regular pool 1; time_trial / mastery: run_for_time_supported true, regular pool 32, ranked deadline 11 s TT / 30 s mastery (5 sets).

## Status: all three modes CORRECT (regular/untimed attempts)
- single_solve f843b1ccdf78b62fb409b322d08007df
- time_trial   ea550bf704eec1c92738e36ad797e64d (695 answers, 0 solver errors; start 4.8 s, submit 4.1 s)
- mastery      d3df859f1fcef196ada5e231ebcac809 (3475 answers = 5 x 695, 0 solver errors; start 5.2 s, submit 5.4 s)
- No code change was needed between modes. Fixtures: `fixtures/{single-solve,time-trial-regular,mastery-regular}`.
- Timed (`--timed`, ranked 11 s TT / 30 s mastery) NOT run: 12-min driver budget; `ranked_attempts_remaining` 32.
  Client-side latency (start ~5 s, submit 4-5 s) makes an 11 s TT window tight - dry-run (`--dry`) on fixtures first.
- Not done: Rust port, regression test script.
