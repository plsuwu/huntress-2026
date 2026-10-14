# Day 14 script-analysis - MSBuild trusted project abuse

Solved natively by `ctf-run` with no lane change (971 answers, CORRECT on first submit): the MSBuild polyglot's
descriptor was found by the existing literal-finder fallbacks. Time trial (1 set) / mastery (2 sets) are ONE-SHOT
(30 s). Fixture `ctf-run/fixtures/script/d14-single_solve.json`.

Time trial and mastery (one-shot, 2026-10-15): both CORRECT first time; graded fixtures in `ctf-run/fixtures/<lane>/d14-{time_trial,mastery}.json`, artifacts under `artifacts/`.
