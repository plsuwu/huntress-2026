## Your task

The first pass above did not finish this lane. Make every mode of your lane CORRECT:

1. Read the errors / oracle output above and the run directories. Implement the fix in your worktree (`{wt}`),
   following the ownership rules from your first message. Prefer generic fixes that follow the question's
   `answer_format` over hard-coded values.
2. Rebuild (`cargo build --release --features tls` in `ctf-run/`), then `./target/release/ctf-run replay fixtures/{fix}`
   plus the new run directories - all clean.
3. Submit, in order, only what is not already CORRECT: `./target/release/ctf-run <single-solve id> -o runs-d{day:02d}`,
   then `... -t -o runs-d{day:02d}` and `... -m -o runs-d{day:02d}` (binary-RE: mastery after a CORRECT TT). Each
   heading above names the route kind: **dual-pool** - plain start is a free untimed regular attempt; **timed by
   default** - plain start is timed (a failure needs `--retry`, which spends another attempt); **ONE-SHOT** - add
   `--one-shot-ok`; it is the route's only attempt, so fire it only after single-solve is CORRECT and the replay
   (fixtures + the new single run) is clean. Never `--timed`. Free single-solve resubmits are the fastest format
   oracle; use the grouped oracle for large answer sets (untimed attempts only).
4. Turn the graded runs into fixtures: `python3 -I tools/fixtures_from_runs.py <run dirs>` (warmups: add `--slug <slug>`).
5. Write `utils/solver_py_scripts/day-{day:02d}-<lane dir>/<slug>/NOTES.md` (what was new, what was graded, gotchas).
6. `git add -A && git commit` on your branch (end the message with the usual Co-Authored-By line).
7. `touch {state}/done-{lane}`, then report in a few lines: what changed, graded findings, anything the coordinator
   must do (shared-file changes, one-shots that failed or were not fired, and why).

Stay on your lane. If you are blocked for more than ~10 minutes on one question, report what you know instead.
