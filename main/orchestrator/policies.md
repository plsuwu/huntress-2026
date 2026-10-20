# Standing policies (user decisions)

Each entry gives the rule, then why, with the date it was set. These override defaults.

## Attempts

- **Timed and ranked attempts need explicit approval, every time.** Approval for one campaign does not carry over to
  the next. Untimed regular attempts on dual-pool routes and single-solve submits are free. *(2026-10-15)*
- **Solves beat Hall of Fame entries.** Fast solves and overall place are the primary goal, and HoF entries come
  second. In the user's words: "we don't care so much if we dont have HoF entries but a good overall place".
  Lane agents and the rush first pass may therefore fire **one-shot** TT/mastery routes (ranked pool 1, no regular
  pool) once single-solve is CORRECT and the replay is clean. One-shots are never HoF-eligible anyway. *(2026-10-18,
  in force from Day 19)*
- **The one-shot risk is accepted only with a feedback loop.** If a timed or one-shot mode fails, it automatically
  gets an untimed `--practice` run on the same artifact. A CORRECT practice run gates the chain on. An incorrect one
  gets the grouped oracle, and the agent keeps iterating with `--practice` (or `--retry` while fresh attempts
  remain). Keep this loop intact in any rush change. *(2026-10-19)*
- **Hold ranked campaigns until every lane is fully solved.** The user held ranked runs until all lanes were merged on
  Day 17 and again on Day 20 ("waiting until all lanes are fully solved before making tt/mastery attempts"). Run one
  campaign over all of the day's routes, not piecemeal per lane. *(2026-10-17, 2026-10-20)*
- **Run ranked attempts from the jumpbox.** If it is down, ask the user to start it; never fall back to running ranked
  attempts locally. *(2026-10-15)*

## Scope

- **Only the five known lanes:** warmups, metadata-forensics, script-analysis, binary-reverse-engineering and
  mobile-android. On Day 20 a **web-exploitation** lane appeared in `op=list` (ids `web-exploitation-day-04-lfi`,
  `web-exploitation-day-05-administratively-speaking`). The user said: "if there are new lanes, you MUST NOT engage
  with them in any capacity". Don't list them for work, start them, or write solvers for them unless the user lifts
  this. *(2026-10-20)*
- **Spawn no sub-agents unless asked.** Rush lane agents are warm parallel Claude instances in tmux. The user asked
  for those instead of sub-agents. *(2026-10-16)*

## Load on shared endpoints

- **Never saturate the GM or a live target.** Several lane agents share one team identity, so a flood can get access
  throttled or blocked for all of them. Defaults:
  - make requests one at a time with a short delay, or with small bounded concurrency;
  - set timeouts, reuse a session, and cache responses locally;
  - no brute-force or fuzzing loops;
  - back off on 429, and never trigger one in the first place.
  
  Do **not** treat a 5xx as throttling and auto-retry or discard it: it may be part of the challenge (an error page
  leaking data), so record it and inspect it. *(2026-10-19)*
- **Probe grading in groups, with visible progress.** The user stopped a blind serial bisection as "taking too long".
  Use grouped oracles (`ctf-run/tools/oracle_groups.py`) and stream their output: no `| tail`. *(2026-10-15)*

## Engineering

- **Solve new days directly in `ctf-run`, in native Rust.** There is no Python-first step for new days, and a Python
  bridge was rejected. Crates are fine where they are clearly faster (e.g. `ring` for SHA-256). *(2026-10-15)*
- **Commit to `master` before a rush starts.** Lane worktrees branch from the committed HEAD.
