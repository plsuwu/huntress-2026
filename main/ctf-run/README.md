# ctf-run

Per-lane solver and low-latency runner for the Huntress CTF 2026 Game Master. Std-only Rust.

```sh
cargo build --release                      # proxy (http) build
cargo build --release --features tls       # + https:// for the external API (HUNTRESS_GM_TOKEN)
tools/deploy.sh                            # copy to the jumpbox, build, test

ctf-run routes --refresh [FILTER]          # cache op=list (8-24 s through the proxy); shows ONE-SHOT routes
ctf-run <challenge> --dry                  # start/reconnect, download, solve, print answers (no submit)
ctf-run <challenge>                        # ... and submit
ctf-run <challenge> -m --retry --timed     # next ranked mastery variant (dual-pool routes; Hall of Fame)
ctf-run replay fixtures/<lane> runs/...    # offline regression against saved answers
ctf-run oracle runs/<challenge>/<run>      # untimed: find which answers are wrong
ctf-run status <challenge> | lanes | bench <zip>
```

Run options: `-t` / `-m` (mode suffix), `--retry`, `--practice`, `--timed`, `-n/--dry`, `--early-submit`, `--partial`,
`--lane NAME`, `-j N` (default: one download per delivery), `-o DIR` (default `./runs`), `-u URL`, `--one-shot-ok`,
`--no-preflight`. stdout: results; stderr: timestamped progress. Every run is saved to
`runs/<challenge>/<mode>-<attempt>-<unix>/` (start, deliveries, answers, result, meta with UTC clock stamps) and can be
replayed directly. See `AGENTS.md` for the design, rules and the new-day recipe.
