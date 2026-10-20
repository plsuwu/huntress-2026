# Coordinator history

Dated outcomes of each release night and campaign. Engineering detail (lane fixes, perf work) lives in
`ctf-run/PLAN.md`'s progress log, and per-day lane gimmicks live in `utils/LANE_PATTERNS.md`. Add a short entry
here after every release night.

## 2026-10-15: ctf-run rewrite; Days 14-15

- **Rewrite:** `ctf-run/` was rewritten from scratch in Rust; the old per-challenge implementation was discarded at the
  user's request. It has per-lane solvers in `src/lanes/`, is std-only, and targets the CourseStack proxy from the
  jumpbox. All five lanes are native, with 231/231 fixtures clean.
- **Day 14:** all 15 routes CORRECT on the first try, including 10 one-shot TT/mastery routes run on the jumpbox over
  ssh.
- **Day 15:** all 15 routes CORRECT untimed, then 5 ranked TT and 5 ranked mastery runs CORRECT. A timing campaign
  (direct API, no preflight) made 358 timed runs, 352 of them CORRECT.
- **Findings:** bandwidth is not the bottleneck; server time-to-first-byte is. Binary-RE mastery is CPU-bound on the
  t3.small.

## 2026-10-16: Day 16, first rush release

- **Why rush exists:** Day 15 took about 35 min serially, against about 10 min for the top teams. The user asked for
  warm parallel instances, so the `huntress-rush.timer` systemd unit started rush at 22:45 every night from 10-16.
- **Release:** detection took 11 s, almost all of it the global `op=list`. Lanes now probe `warmups-day-NN` status
  every 1 s and fetch their own routes with `search=`.
- **Lanes:** the binary agent's untimed `-t` was blocked by the permission classifier, so it stopped without a done
  marker. Fixed with `--allowedTools`. The first pass now runs its modes concurrently (user-approved: a wrong solver
  only costs untimed regular variants).
- **Ranked:** #1 on 6 of 10 routes by 23:22.
  - Behind: warmups TT #2 (811 vs 614, server-bound), metadata TT #2 (709 vs 629, server-bound), binary TT #2 (1900 vs
    1899) and binary mastery #3 (4886 vs 4465, 13 attempts left).
  - The user deferred the second pass to a full ranked re-run the next afternoon, 2026-10-17 ~14:00-16:00 AEST.
  - Logs: `ctf-run/runs-hof/campaign-d16-{a,b,c}.log`.

## 2026-10-17: Day 17

- **Release:** the release probe counted `409 start_required` as not released, which cost 9 s (fixed in 8a96837).
- **New gate:** TT/mastery are gated on a CORRECT single-solve, so a concurrent first pass only helps single-solve.
  Chaining the modes came later.
- **Ranked:** held until every lane was done, then one campaign. #1 on 9 of 10 by 23:45.
- **Binary roll-up:** labels took about 25 min of free probing (see `grading-lessons.md`). `oracle_groups.py` now
  groups roll-up ids by day (eb32b48).

## 2026-10-18: Day 18

- **One-shots:** warmups and binary TT/mastery were one-shot routes, which are not HoF-eligible. All four were fired
  after merge and all were CORRECT.
- **All 15 CORRECT** by 23:12. Ranked: #1 on 6 of the 6 dual-pool routes.
- **Full pass over every day:** 139 CORRECT; #1 went from 26 to 36 of 152 routes.
- **Re-grade:** the GM re-graded Batch expansion at about 13:30Z (see `grading-lessons.md`). The user then made solves
  outrank HoF entries (see `policies.md`).

## 2026-10-19: full pass; Day 19

- **Afternoon full pass** (16:25-17:00): 137/138 CORRECT, plus one 503 that was re-run. #1 went from 35 to 38.
- **Jumpbox:** it had been auto-powered-off since about 11:40 AEST.
- **Day 19:** released at 23:00:01.
  - Mobile and script were CORRECT on the first pass in under 1 min.
  - The warmups, metadata and binary agents were done by 23:07.
- **Campaign:** `plan-d19.json`, budget 3 (2 for the 16-attempt pool). #1 on 9 of 10 by about 23:25.

## 2026-10-20: Day 20; automation torn down

- **Before release:** the VPN and jumpbox were down at 22:55, and the user restarted them.
- **Release** at 23:00:01:
  - Warmups and script: all modes CORRECT on the first pass in under 40 s.
  - Agents fixed the other three by about 23:05:
    - mobile: a `dynamic` codec that follows the puzzle path to a secondary DEX;
    - metadata: Mach-O `cpu-type-set`, thin and fat slices;
    - binary: the RS20 resource overlay and `hcfg_resource_size`.
- **New lane:** **web-exploitation** appeared and was left alone, per the user's instruction.
- **Merge and deploy:** merged and deployed with 314/314 replay clean.
- **Campaign** (`plan-d20.json`, held until all lanes were solved): 13 runs, all CORRECT, **#1 on 8 of 10**.
  - Binary mastery: #3 (4105 vs 3764). Budget 2, 14 attempts left.
  - Warmups TT: #5 (735 vs 484). 253 attempts left; server-bound, so re-roll in the 16-21h window.
- **Teardown:** the user then removed the `huntress-rush` and `huntress-netprobe` systemd timers. Rush is now started
  by hand (see `runbook.md`). The netprobe findings are in `hall-of-fame.md`.
