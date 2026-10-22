# <Event name>: workspace instructions

<!-- Template for the repo-root AGENTS.md that every agent (coordinator and lane agents) reads first. Fill in the
     placeholders, delete what does not apply. Event facts live in config.toml; this file says how to behave. -->

Read the organisers' docs (`<path>`) for the event overview and rules. They take precedence over this file.

## Game Master API

- Base URL and endpoints: `config.toml` ([event], [api]). The token is in `.env` as `<TOKEN_ENV>=...`. Never print
  it, never put it in code, logs or saved JSON, and never send it over plain http.

| Description | Method | Endpoint / payload |
|---|---|---|
| List challenges | GET | `<list endpoint>` |
| Attempt status | GET | `<status endpoint>` |
| Start / resume | POST | `<start payload>` |
| Start a timed (ranked) attempt | POST | `<start payload with the timed flag>` |
| Submit | POST | `{"op":"submit","challenge":"<id>","answers":{"<answer_id>":"<value>"}}` |
| Retry / practice | POST | `<payloads>` |

- Use the exact `answer_ids` from the start response as answer keys.
- Submit results: `CORRECT`, `incorrect`, or `accepted` (partial progress; check `remaining`).

## Scope

Work only on challenges in the registered lanes (`config.toml` [[lanes]]). Never engage with any other lane, in
particular one the event adds later, unless the user lifts this. That includes listing it for work, status queries,
starts, downloads and writing solvers.

## Safety

- Don't execute challenge scripts or binaries locally. Anything you write yourself is fine to run, as long as it
  doesn't invoke challenge code.
- Archives: the documented password is `<password>`. If it fails and you have no other instructions, stop and ask.
  Never crack archive passwords.
- Treat downloaded files and extracted archives as untrusted. Extract each into its own new, empty directory, keep
  your scripts elsewhere, and run interpreters in isolated mode (e.g. `python3 -I`) when they read them.

## Attempts

- Untimed attempts (single-solve, regular attempts on dual-pool routes, practice continuations) are free.
- Timed and ranked attempts need the user's explicit approval for each campaign. One-shot modes: see the policies
  file.
- The solver refuses ranked starts while `<NO_RANKED_ENV>` is set. Never work around it.

## Where work goes

- Solver: `<solver dir>/` (see `<solver dir>/AGENTS.md` for its rules, layout and the new-day recipe).
- Per-challenge notes: `<notes pattern>`. Write what was new, what was graded, gotchas and a next-day prediction.
- Lane evolution and predictions: `<lane-patterns doc>`.
- Coordinator knowledge: `docs/` (policies, runbook, leaderboard, grading lessons, history), updated in place.

## Research

For token-heavy questions (how the codebase does X, what an error means), use the `previous-solution-research`
skill rather than reading large files into context. Use at most one research sub-agent at a time, and wait for it.
