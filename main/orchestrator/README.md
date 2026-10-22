# orchestrator

Knowledge for the **coordinator** session, the one that runs a day's release. Lane agents solve; the coordinator
watches them, merges, deploys and spends ranked attempts. Everything here was learned during the event (it used to
live in per-machine agent memory) and is now the single source of truth. Update these files in place, and add
dated entries to `history.md`, rather than writing new notes elsewhere.

| File | What it holds |
|---|---|
| `policies.md` | Standing user decisions: what needs approval, what to leave alone, solves vs Hall of Fame |
| `runbook.md` | Release night, step by step: rush, monitoring, merge, deploy, ranked campaign, jumpbox |
| `hall-of-fame.md` | Speed-scoreboard API, what `elapsed_ms` measures, campaign tooling, time-of-day latency |
| `grading-lessons.md` | Game Master / grader behaviour that burned attempts, and how to work around it |
| `history.md` | Dated log of each release night and campaign (Days 14-20) |

Related docs elsewhere in the repo:

- `AGENTS.md`: the instructions every agent reads (GM API, solution layout).
- `HOMEBASE.md` and `TIMETRIAL-MASTERYMODE.md`: the organisers' docs, kept verbatim.
- `ctf-run/AGENTS.md`: the Rust solver's rules, its layout and the new-day recipe for a lane.
- `ctf-run/PLAN.md`: the solver's design, measurements and engineering progress log.
- `utils/LANE_PATTERNS.md`: how each lane evolves day to day, with predictions.
- `utils/solver_py_scripts/day-NN-<lane>/<slug>/NOTES.md`: per-challenge notes.

Event-independent versions of this knowledge (no lane, day or host names), for starting over elsewhere:

- `../playbooks/solver-runner.md`: building, extending, benchmarking and deploying a compiled solver binary.
- `../playbooks/orchestrator.md`: the coordinator's job, covering the lane registry, release flow, agents, campaigns and monitoring.
