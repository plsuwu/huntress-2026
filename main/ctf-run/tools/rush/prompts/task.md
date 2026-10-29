## Your task

The first pass above did not finish this lane. Make every mode of your lane CORRECT:

1. Read the errors / oracle output above and the run directories. Implement the fix in your worktree (`{wt}`),
   following the ownership rules from your first message. Prefer generic fixes that follow the question's
   `answer_format` over hard-coded values.
2. Rebuild **without `--release`** (`cargo build --features tls` in `ctf-run/`; the release profile's LTO build is
   far slower, and the debug cache is warm). Keep `--features tls` (without it the binary refuses https). Iterate with
   the debug binary; only before firing a timed-by-default or ONE-SHOT attempt (timed, so analysis speed counts)
   rebuild once with `cargo build --release --features tls` and fire it with `./target/release/ctf-run`.
   **Do not run `replay` here or anywhere before step 4** - not between submits, not between modes, not before a
   one-shot. Speed matters; the GM's verdict on the live attempt is the check.
3. Submit, in order, only what is not already CORRECT: `./target/debug/ctf-run <single-solve id> -o runs-d{day:02d}`,
   then `... -t -o runs-d{day:02d}` and `... -m -o runs-d{day:02d}` (binary-RE: mastery after a CORRECT TT). Each
   heading above names the route kind: **dual-pool** - plain start is a free untimed regular attempt; **timed by
   default** - plain start is timed (a failure needs `--retry`, which spends another attempt); **ONE-SHOT** - add
   `--one-shot-ok`; it is the route's only attempt, so fire it only after single-solve is CORRECT. Never `--timed`.
   Free single-solve resubmits are the fastest format oracle; use the grouped oracle for large answer sets (untimed
   attempts only).
   **A timed-by-default or ONE-SHOT mode that is not CORRECT** (above: its timed result, then a `practice` line):
   the first pass already ran its untimed practice continuation (`--practice`: same artifact, no clock, free) and,
   if that was incorrect, the grouped oracle on it. Iterate there on your own: fix, rebuild, re-run
   `./target/debug/ctf-run <single-solve id> -t|-m --practice -o runs-d{day:02d}` (it reopens the same attempt) and
   `python3 -I tools/oracle_groups.py <practice run dir>` until it is CORRECT, then confirm with `op=status` that the
   route shows CORRECT. If the GM refuses practice because fresh attempts remain (timed by default, pool > 1),
   validate the fix with a free single-solve resubmit, then `--retry` - it spends one of
   those attempts, which is fine. A failed run dir is the evidence: its artifacts and submitted answers are saved.
4. Only now, with every mode CORRECT (or reported as blocked): one regression pass,
   `./target/debug/ctf-run replay fixtures/{fix}` plus this day's run directories - all clean. If it shows a regression
   in an older fixture, fix it and re-run the replay; the day's modes are already graded, so do not resubmit.
5. Turn the graded runs into fixtures: `python3 -I tools/fixtures_from_runs.py <run dirs>` (warmups: add `--slug <slug>`).
6. Write `utils/solver_py_scripts/day-{day:02d}-<lane dir>/<slug>/NOTES.md` (what was new, what was graded, gotchas).
7. `git add -A && git commit` on your branch (end the message with the usual Co-Authored-By line).
8. `touch {state}/done-{lane}`, then report in a few lines: what changed, graded findings, anything the coordinator
   must do (shared-file changes, one-shots that failed or were not fired, and why).

Stay on your lane: challenges starting `{prefix}` only, never web-exploitation or any other new lane. If you are blocked for more than ~10 minutes on one question, report what you know instead.
