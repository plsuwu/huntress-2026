## Your task

The first pass above did not finish this challenge. Make every mode of `{chal}` CORRECT:

1. Read the evidence above and the mode directories under `{wt}`. Reconnect to the current
   attempt with `python3 tools/wk.py begin <mode>` (idempotent - it reconnects to the same
   attempt, never starts a second one).
2. Solve whatever is not CORRECT, in order: single_solve, then time_trial, then mastery.
   Submit with `python3 tools/wk.py submit <mode>` as soon as you have answers; on CORRECT,
   begin the next mode at once. (`python3 tools/wk.py status` shows every route's GM state;
   `python3 tools/wk.py next` names the first route that is not CORRECT.)
3. On `incorrect` or `accepted`: this is an untimed attempt, so partial submits are a free
   oracle - submit subsets of your answers and read `accepted_ids`/`remaining` to localise
   what is wrong. Re-derive each wrong answer from evidence; never cycle through guesses.
4. Mastery assigns multiple targets mixing earlier bug classes. Classify every target
   independently (join each question's `artifact` field to `targets[].id`) - do not assume
   the challenge's own bug.
5. Write `solver_py_scripts/day-{day:02d}-web-exploitation/{slug}/NOTES.md`: what was new,
   what was graded, gotchas. Keep `solve.py` generic - derive routes from the live target
   (e.g. `/api/clues`), never hard-code per-attempt answers. If you needed a bug class
   `recovery_lib.py` does not know, document the method precisely in NOTES.md; the
   coordinator folds new recoverers into the shared library.
6. Commit your day directory - `git add solver_py_scripts/day-{day:02d}-web-exploitation/{slug} && git commit`, ending the message with the usual Co-Authored-By line.
7. `touch {state}/done-{short}`, then report in a few lines: what changed, graded findings,
   anything the coordinator must do.

Stay on your challenge: routes under `{chal}` only, never any other lane. If you are blocked
for more than ~10 minutes on one question, record the exact reason in NOTES.md and move on.
