# CTF orchestration template

A clean-slate kit for running a CTF with a compiled solver binary, one warm agent per **lane** (a family of
challenges sharing an id prefix), automated release-night passes and leaderboard campaigns. Nothing in here names a
real event, lane, host or day: all of that lives in one config file.

Read the two playbooks first. They explain *why* everything here is shaped the way it is:
- `playbooks/solver-runner.md`: building, extending, benchmarking and deploying the solver.
- `playbooks/orchestrator.md`: the coordinator's job, covering the lane registry, release flow, agents, campaigns
  and monitoring.

## Layout (copy this directory to the root of a new repo)

| Path | What it is |
|---|---|
| `config.example.toml` | **The only event-specific file.** API, team, release time, solver binary/flags, agent settings, run host, campaign knobs, and the lane registry. Copy it to `config.toml` |
| `AGENTS.example.md` | Template for the repo-root `AGENTS.md` that every agent reads first: API table, scope, safety, attempt rules |
| `tools/orchcfg.py` | Config loader used by every tool (Python import, or a CLI for shell scripts: `get`, `lanes`, `fmt`, `check`) |
| `tools/rush/rush.py` | Release engine: `prep` (worktree per lane, warm builds), `launch` (one agent per tmux window, pinned model, allowlisted solver, heartbeat/stop hooks), `watch` (release detection, automated first pass, briefs, stall detection), `status`, `takeover`, `--rehearse` |
| `tools/rush/prompts/` | `prime.md` (agent startup and session rules), `task.md` (lane not finished), `task-done.md` (lane already solved). Placeholders are filled from the config |
| `tools/merge.sh` | Merge every lane with a done marker, then build + tests + full replay |
| `tools/deploy.sh` | Static build, ship over one multiplexed ssh connection, keep the previous binary, atomic swap |
| `tools/survey.py` | Attempts left x leaderboard for in-scope routes; `--plan` writes a campaign plan |
| `tools/campaign.py` | Resumable ranked campaign with stopping rules (#1, budget, patience, any non-CORRECT) |
| `tools/oracle_groups.py` | Grouped partial-submit oracle on untimed attempts (family, then field) |
| `tools/fixtures_from_runs.py` | Graded run directories -> permanent replay fixtures |
| `tools/run_legs.py` | Per-leg timings of saved runs against the server clock |
| `tools/bench.sh` | Before/after benchmark of two solver binaries (suite + fresh-process fixtures) |
| `tools/nested_zip.py` | Nested-archive registry and `"a.zip -> member"` label resolution |
| `skills/previous-solution-research/` | Skill: send a self-contained question to a cheap isolated sub-agent, get a short cited answer back |
| `skills/lane-skill.example/` | Skeleton for a per-lane analysis skill (explain vs solve, safety, classification discipline, answering) |
| `docs/` | Skeletons: `policies.md`, `grading-lessons.md` (seeded with lessons that held across events), `history.md`, `lane-patterns.md`, `runbook.md` |
| `solver-reference/` | Rust excerpts from a working solver: `lane_contract.rs` (the lane trait), `guards.rs` (fail-closed start guards, with tests), `rx.rs` (Python `re` semantics on fancy-regex) |

## Bootstrapping a new event

1. Copy this directory to a new repo root. Then:
   - `cp config.example.toml config.toml`;
   - `cp AGENTS.example.md AGENTS.md`;
   - copy `skills/*` to `.claude/skills/`.
2. Fill in `config.toml`: [event], [api], [modes], [solver], [agents], [run_host]. Leave the lane registry empty for
   now, apart from the first lane.
3. Build the solver (see `playbooks/solver-runner.md` §5), with these properties:
   - It persists every run as `meta.json`, `start.json`, `deliveries/`, `answers.json` and `result.json`.
   - It has a `replay <fixtures dir>` command whose last line ends in `<ok>/<total> case(s) clean`, or change that
     check in `merge.sh` and `bench.sh` to match your output.
   - It honours the no-ranked environment variable. Port `solver-reference/guards.rs`.
   - It prints the final submit response as one JSON line on stdout, and `submit <RESULT>` and `saved <dir>` lines
     on stderr. Otherwise adjust `[solver.result]`.
4. `python3 tools/orchcfg.py check`.
5. Fill in `docs/policies.md` with the user's answers: approvals, scope, solves vs leaderboard, where ranked runs
   happen.
6. Rehearse on an already-released day: `python3 -u tools/rush/rush.py all --day N --rehearse`.

## Adding a lane

1. Add a `[[lanes]]` row to `config.toml` with `name`, `prefix`, `source`, `fixtures`, and optionally `hedge`,
   `release_probe`, `slug` and `sequential`.
2. Add a lane module that implements the contract. It should fail loudly with evidence on every unknown kind.
3. Create `fixtures/<fixtures>/` under the solver, and a skill from `skills/lane-skill.example` if the lane needs
   one.
4. Solve its single-solve by hand until CORRECT, make fixtures with `tools/fixtures_from_runs.py`, and get a 100%
   replay.
5. Rehearse: `rush.py watch --day N --rehearse --lanes <name> --no-agents`.

Only registered lanes are ever touched. The survey, campaign, oracle, fixture tools and release watcher all filter
challenge ids through the registry, and the agent prompts forbid anything else.

## Things the tools assume (change them in one place if your platform differs)

- **Route ids:** `<prefix><NN>[-slug]` for the untimed mode, `<base>/<mode>` for timed modes ([modes]).
- **Route kinds**, from the challenge list:
  - `run_for_time_supported`: dual-pool. Plain start is untimed; ranked is opt-in.
  - otherwise timed by default, and one-shot when `ranked_pool_size == 1`.
  - See `route_kind()` in `rush.py`.
- **Status errors:** `409` (`start_required`, `prerequisite_required`) means released but not started. The
  configured `unreleased_error` means not released yet.
- **One-shot modes:** automation fires them once single-solve is CORRECT ("solves first"). Change `mode_args()` in
  `rush.py` if your policy differs.
