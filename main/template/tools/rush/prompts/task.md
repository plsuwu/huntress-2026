## Your task

The first pass above did not finish this lane. Make every mode of your lane CORRECT:

1. **Fix.** Read the errors and oracle output above and the run directories, then implement the fix in your
   worktree (`{wt}`), following the ownership rules from your first message. Prefer generic fixes that follow the
   question's declared answer format, but when the artifact itself states the exact answer, submit what it states.
2. **Rebuild and replay.** Rebuild in `{solver}/`, then run `./{bin} replay fixtures/{fix}` plus the new run
   directories. All must be clean.
3. **Submit**, in order, only what is not already CORRECT:
   - `./{bin} <{single} id> {flags[out]} {runs}`;
   - then each timed mode ({modes}) with its mode flag. Modes the brief marks as waiting on another mode run after
     it.

   Each heading above names the route kind:
   - **dual-pool:** a plain start is a free untimed regular attempt. Never add `{flags[timed]}`.
   - **timed by default:** a plain start is timed. A failure needs `{flags[retry]}`, which spends another attempt.
   - **ONE-SHOT:** add `{flags[one_shot_ok]}`. It is the route's only attempt, so fire it only after {single} is
     CORRECT and the replay (fixtures plus the new {single} run) is clean.

   Free {single} resubmits are the fastest format oracle. Use the grouped oracle for large answer sets, on untimed
   attempts only.
4. **A timed-by-default or ONE-SHOT mode that is not CORRECT** (above: its timed result, then a `practice` line):
   - The first pass already ran its untimed practice continuation (`{flags[practice]}`: same artifact, no clock,
     free) and, if that was incorrect, the grouped oracle on it.
   - Iterate there on your own: fix, rebuild, re-run `./{bin} <{single} id> <mode flag> {flags[practice]} {flags[out]} {runs}`
     (it reopens the same attempt) and `python3 -I {repo}/tools/oracle_groups.py <practice run dir>` until it is
     CORRECT. Then confirm with a status query that the route shows CORRECT.
   - If the server refuses practice because fresh attempts remain, validate the fix (fixtures, {single}, the failed
     run's saved artifacts), then use `{flags[retry]}`. It spends one of those attempts, which is fine.
5. **Fixtures:** turn the graded runs into fixtures with `python3 -I {repo}/tools/fixtures_from_runs.py <run dirs>`
   (add `--slug <slug>` if the challenge id has none).
6. **Notes:** write the challenge's notes file: what was new, what was graded, gotchas, and a prediction for
   tomorrow.
7. **Commit:** `git add -A && git commit` on your branch.
8. **Report:** `touch {state}/done-{lane}`, then report in a few lines: what changed, graded findings, and anything
   the coordinator must do (shared-file changes, one-shots that failed or were not fired, and why).

Stay on your lane: challenges starting `{prefix}` only. If you are blocked for more than about 10 minutes on one
question, or a command is refused, report what you know instead.
