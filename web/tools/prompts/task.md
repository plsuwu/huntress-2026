## Your task

The first pass above did not finish this challenge. Make every mode of `{chal}` CORRECT:

1. Read the evidence above and the mode directories under `{wt}`. Reconnect to the current
   attempt with `python3 tools/wk.py begin <mode>` (idempotent - a plain untimed `start` that
   reconnects to the same attempt and never spends a second timed attempt). If the live target
   becomes unreachable mid-attempt (500 / Cloudflare 1101 / 410 / connection refused /
   crashed), the recovery is to re-run `python3 tools/wk.py begin <mode>`: that re-provisions
   a fresh target under the attempt (and transparently picks up a new attempt if the old one
   expired, warning you to re-derive any stale answers). Do NOT sit polling a dead target
   waiting for it to auto-restart - re-running `begin` is a plain untimed start, not a
   reset/switch_target. Better still, avoid crashing it: send well-formed requests only, since
   a malformed/non-object body can 500 the origin and take the container down for the attempt.
2. Solve whatever is not CORRECT, in order: single_solve, then time_trial, then mastery.
   Submit with `python3 tools/wk.py submit <mode>` as soon as you have answers; on CORRECT,
   begin the next mode at once. (`python3 tools/wk.py status` shows the local record per
   route, `status --live` the GM's current view; `python3 tools/wk.py next` names the
   first route that is not CORRECT.)
3. On `incorrect` or `accepted`: this is an untimed attempt, so partial submits are a free
   oracle - submit subsets of your answers and read `accepted_ids`/`remaining` to localise
   what is wrong. Re-derive each wrong answer from evidence; never cycle through guesses.
4. Mastery assigns multiple targets mixing earlier bug classes. Classify every target
   independently (join each question's `artifact` field to `targets[].id`) - do not assume
   the challenge's own bug.
5. Write `solution-notes/day-{day:02d}-{slug}.md` (symlinked into your directory - the real
   path is `workers/web/solution-notes/`, the consolidated, tagged reference library every
   worker shares). Use the YAML frontmatter template already at the top of any existing
   `solution-notes/day-*.md` file (`day`, `slug`, `lane: web-exploitation`, `modes`,
   `has_solve_py`, `tags: []`) - fill in everything except `tags`, which you should actually
   fill in yourself this time (short kebab-case bug-class/technique tags - see
   `solution-notes/TAGGING-PROMPT.md` for the convention other notes were tagged with). Below
   the frontmatter, write what was new, what was graded, gotchas. Keep `solve.py` generic -
   derive routes from the live target (e.g. `/api/clues`), never hard-code per-attempt
   answers - then copy it verbatim into `past-solutions/day-{day:02d}-{slug}-solve.py`
   (symlinked the same way; name any extra scripts, e.g. a separate mastery solver,
   `past-solutions/day-{day:02d}-{slug}-mastery.py`). If you needed a bug class
   `recovery_lib.py` does not know, document the method precisely in your note; the
   coordinator folds new recoverers into the shared library.
6. Commit it - `git add solution-notes/day-{day:02d}-{slug}.md past-solutions/day-{day:02d}-{slug}-*.py && git commit`, ending the message with the usual Co-Authored-By line (the symlinked relative paths resolve to their real location under `workers/web/` - `git add` follows them correctly).
7. Report in a few lines: what changed, graded findings, anything the coordinator must do, then
   stop - the driver marks your challenge done once this turn ends.

Stay on your challenge: routes under `{chal}` only, never any other lane. If you are blocked
for more than ~10 minutes on one question, record the exact reason in NOTES.md and move on.
