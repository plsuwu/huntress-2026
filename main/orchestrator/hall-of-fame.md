# Hall of Fame

## Scoreboard API (undocumented)

It is not part of the GM `op=` API or its OpenAPI description; it was found via the browser's leaderboard page. It uses
the same host and Bearer token:

```
GET https://2026.huntress.ctf.games/api/speed-scoreboard?mode=mastery&view=rankings&limit=3&offset=0&sort=elapsed_ms&direction=asc&challenge=<base id>
```

| Parameter | Meaning |
|---|---|
| `mode` | `time_trial` or `mastery` |
| `view` | `rankings` |
| `limit` / `offset` | paging (`limit=200` returns every team on a route) |
| `sort` / `direction` | `elapsed_ms` / `asc` |
| `challenge` | the **base** challenge id, without the `/time_trial` or `/mastery` suffix |

Each row carries `team_name` (ours is **`help`**), `challenge_id`, `rank`, `elapsed_ms`, `par_ms`, `under_par_ms`,
`answer_count`, `set_count` and a **`cohort`** hash. Rows have **no timestamp**: only the response's `captured_at`,
plus a `coverage` block ("prospective_qualified_fresh_attempts", "fastest_eligible_fresh_attempt_per_participant",
backfill pending; earlier attempts without eligibility evidence are never added). To date an entry, diff surveys
over time (`hof_survey.py --json` now records `cohort`, `par` and the top-3 cohorts). Cohorts differ between teams
on one route and one ranking mixes them: on d10 mastery our entry and the 113 ms leader share cohort `21da…` (par
200 s, the route's old timed-by-default pool) while #2/#3 are in `6d12…` (par 28 s). So a cohort looks like the
pool/generation an attempt came from, not a separate race.
Pair the scoreboard with `?op=status&challenge=<id>/<mode>`, which gives `ranked_attempts_remaining` and
`hall_of_fame_eligible`, to decide where ranked attempts are worth spending.

## What `elapsed_ms` measures

`elapsed_ms` is roughly (submit sent - `started_at`) plus the server's grading time. The scoring round trip after
grading is not counted. The server timer starts at the end of `start` processing, so start latency is free. Every ms
between `started_at` and the submit counts.

- **Server-bound routes** (warmups, mobile, metadata: tiny downloads). Elapsed is roughly start lag (~70 ms, up to
  ~365) + the slowest download + ~200 ms grading. Ranked downloads straggle more than regular ones, so use `--hedge`
  (duplicate downloads) there, never on binary-RE (32 MB). Past that, re-rolls are luck.
- **Binary-RE** is the one lever engineering can pull: analysis is ~600 ms per TT and ~2.6 s per mastery on the
  jumpbox. On Day 15 our own analysis CPU (1.1 s), not the network, was the gap to the leaders. Compare our legs
  (`ctf-run/tools/run_legs.py <run dirs>`) against the leaders before re-rolling.
  - **Downloads are capped per connection** at ~8-25 MB/s (never >30), on every day and hour measured (161 ranked
    runs, Oct 22-26), while 8-9 parallel connections reach 90-113 MB/s. It is upstream (Cloudflare/origin), not t3
    network credits or our TCP. A 36 MB TT zip takes ~2 s.
  - **No HTTP Range:** `Range: bytes=0-15` returns `200` and the whole body (no `Accept-Ranges`), so a download
    cannot be split across connections.
  - Since 2026-10-26 analysis overlaps the download (streaming members, progressive d08): TT analysis after the
    last byte went 532 -> 96 ms. Mastery stays CPU-bound (~5 s of analysis for 9 sets on 2 hyperthreads).
- **Implausible leader times** (d10 mastery 113 ms on 95 MB, d13 TT 155 ms and mastery 224 ms on 32 MB) cannot
  include a full download: the download TTFB alone is 150-250 ms. They are one submit round trip, i.e. answers
  known before the clock. The d13 ones were on the board by Oct 16, when the board was new and had 7-11 teams;
  the d10 one appeared between Oct 16 and 22. Our own 337 saved attempts never got the
  same artifact twice, and `started_at` is always stamped before the start response leaves (35-1329 ms before we
  receive it), so neither an answer cache nor a late clock is available to us. Don't chase these.
- **Cold starts:** the first request after an idle period costs 1.6-2.3 s against 0.25-0.45 s warm. A warm-up request
  before a ranked start probably doesn't help, since `started_at` is likely stamped after any proxy cold boot. This
  was never tested against `elapsed_ms`.

## Jumpbox

`HuntressCTF@10.1.181.90` (key `huntress-id_ed25519` in the repo root; token in `~/.huntress_token` on the box).

- It is a t3.small: 2 vCPUs that are hyperthreads of one core (Xeon 8259CL: AES, AVX2, AVX-512, **no SHA-NI**),
  with burstable credits. Steal time stayed negligible during campaigns.
- **Memory: 1.9 GB, no swap, ~450 MB always taken** by the organisers' services (`huntress-ai-arena` ~270 MB,
  docker, snapd), so ~1.2 GB is available. On 2026-10-26 a 9-set binary mastery peaked ~1.8 GB and was
  OOM-killed mid-run, losing a ranked attempt (`dmesg`/`journalctl -k` are not readable there; the tell is
  `rc -9` in the campaign jsonl). ctf-run now caps it at ~0.8-0.9 GB; re-measure if mastery sets keep growing.
- **Disk: 38 GB.** `runs-hof/` grows ~6 GB per full pass (binary zips); see the runbook's disk check.
- It has no root (sudo wants a password) and runs Ubuntu with glibc 2.39.
- It is powered off automatically after long uptime. When that happens ssh times out while the VPN gateway still
  pings; ask the user to start it.
- The crate lives in `~/ctf-run-v2`. Ship the static musl binary from the repo flake with `ctf-run/tools/deploy.sh`
  (uploads `ctf-run.new`, renames it into place, keeps `ctf-run.prev`). Never compile there before ranked runs: it
  burns the CPU credits that the timed runs need.
- To try a build without touching production, copy it as e.g. `ctf-run.stream` and replay saved run dirs there
  (`CTF_REPLAY_MBPS=17` paces the feed like a download; `CTF_REPLAY_STEP=16384` uses live-sized reads), or `--dry`
  a solved route: a plain start reconnects to the finished attempt and submits nothing.

## Tooling

| Tool | Use |
|---|---|
| `ctf-run/tools/hof_survey.py --days A-B [--json OUT]` | Route status x scoreboard: our rank and time vs the leaders. Routes come from the local `ctf-run routes` cache |
| `ctf-run/tools/hof_campaign.py PLAN OUT.jsonl [DONE,...]` | Runs on the jumpbox and is resumable. Per route: up to `budget` runs; stops on #1, after 3 non-improving runs, on no attempts left, or on any non-CORRECT result. Backs off on 503 |
| `ctf-run/tools/hof_plan.py SURVEY OUT [--days A-B] [--unattempted] [--exclude PLANS]` | Survey -> plan with the standard budgets; skips HoF-ineligible routes |
| `ctf-run/tools/prune_hof_runs.sh [--do]` | Frees jumpbox disk: drops old binary-RE evidence zips from `runs-hof/` (run it via `ssh ... 'bash -s'`) |
| `ctf-run/runs-hof/` | Plans, surveys and campaign logs/jsonl |

Every campaign run is a real ranked attempt: `--retry --timed` on dual-pool routes, or `--retry --one-shot-ok` on
timed-by-default ones. Late-published ranked pools need `retry` with `run_for_time:true`: Day 14 became dual-pool after
its one-shot day. Re-survey old days now and then, because routes can change after the fact.

## When to run: GM latency by hour of day (AEST)

The netprobe timer took one burst from the jumpbox every 10 min, from 2026-10-18 00:25 to 2026-10-20 23:15. It made
413 records, 283 of them usable bursts. Timings are warm time-to-first-byte p50/p90 in ms, through the CourseStack
proxy:

| Hour | Download | Status | | Hour | Download | Status |
|---|---|---|---|---|---|---|
| 00 | 280/900 | 330/714 | | 12 | 223/281 | 295/858 |
| 01 | 261/921 | 340/1179 | | 13 | 214/299 | 261/492 |
| 02 | 270/564 | 337/1028 | | 14 | 204/452 | 252/671 |
| 03 | 255/540 | 332/648 | | 15 | 200/285 | 252/740 |
| 04 | 275/858 | 336/1313 | | 16 | 181/357 | 222/430 |
| 05 | 277/454 | 332/746 | | 17 | 182/268 | 225/465 |
| 06 | 240/395 | 295/670 | | 18 | 195/244 | 232/299 |
| 07 | 250/488 | 314/566 | | 19 | 199/257 | 238/445 |
| 08 | 236/400 | 286/603 | | 20 | 205/302 | 252/357 |
| 09 | 244/288 | 299/477 | | 21 | 206/293 | 258/393 |
| 10 | 238/534 | 296/812 | | 22 | 246/493 | 293/806 |
| 11 | 232/378 | 291/619 | | 23 | 299/505 | 330/528 |

**Confirmed 2026-10-26:** a full pass from 15:30, with warmups/mobile/metadata first (`hof_plan.py --order`), took
#1 from 81 to 102 (metadata +8, warmups +4), where the release-night pass the night before had gained only 2.

**Takeaway:** 16:00-21:00 AEST is the quietest window, with download p50 at 180-205 ms and p90 under ~360 ms. Release
night (22:00-05:00) is the slowest, with p90 often 500-900 ms. So server-bound HoF re-rolls are best done in the late
afternoon. Hours 12-16 had only about 6 samples each. The raw data is `ctf-run/runs/netprobe/probes.jsonl`
(gitignored); the probe tooling was removed with the timer on 2026-10-20 and is in git history (`ctf-run/tools/netprobe/`).
