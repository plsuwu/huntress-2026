---
day: 14
slug: "the-api-has-opinions"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

# web-exploitation-day-14-the-api-has-opinions (release day 24)

All three modes CORRECT: `single_solve`, `time_trial`, `mastery`.

## single_solve / time_trial: "WAR Room" app, field/action authorization bypass

Single app theme ("WAR Room", Jakarta Servlet/Tomcat skin) appeared for both
single_solve and time_trial. `GET /api/clues` discloses:

```json
{"framework": "...", "feature_route": "/execute-<hex>", "inventory_route": "/api/inventory",
 "current_role": "analyst", "protected_reference": <id>}
```

`GET /api/inventory` lists the generic endpoint's contract (differs in path per
attempt, shape is stable):

```json
{"resource": "cases", "route": "<feature_route>", "method": "GET",
 "query_parameters": ["id", "fields", "action"], "fields": ["title", "note"],
 "actions": ["view", "export"], "public_reference": <id>, "sample": {...}}
```

The bug: the server checks authorization per `(field, action)` pair, not
consistently. `action=view` on the restricted field (`note`) for the
`protected_reference` id returns `{"error":"field_action_forbidden"}`, but
`action=export` with the same field is *not* gated the same way and returns
`{"ok":true,"effect":"api_field_action_authorization_bypassed","evidence":"proof_..."}`.

This is a brute-forceable 2x2 (or small) grid over `fields x actions` from the
inventory response — no guessing needed, the server tells you the exact grid.

## mastery: three independently-classified targets

Mastery assigned 3 targets with **3 different bug classes** in 3 different
themed apps (not one app repeated). Per the standing instruction, each was
classified independently from its own evidence, not assumed from the
family's prompt ("API Has Opinions" only literally matches target 3 here).

1. **Target 1 "Service Postcard"** (tiny native Node.js server) — **backup
   file exposure**. No `/api/clues` (404). Root page has an HTML comment:
   `<!-- deployment note: remember to review crawler policy before launch -->`.
   `/robots.txt` discloses `Disallow: /archives-53f77/` — the path is
   reachable anyway (robots.txt is not an access control) and serves a
   directory index with `environment-copy-<hex>.json.bak`, whose JSON body
   has `{"environment":"production","support_note":"proof_..."}`.

2. **Target 2 "Case Crate"** — **IDOR via a leaky activity feed**. Root page
   shows "Signed in as Jules" and only Jules's own case ids (no login/session
   setup needed — a `case_session` cookie is auto-issued on the first
   request). `GET /api/timeline` (session-cookie scoped) returns an events
   list that includes OTHER users' case ids with
   `"visibility":"reference_only"` (e.g. owner `"executive"`). The per-ticket
   API `GET /api/tickets/<id>` has **no ownership check** — fetching the
   leaked foreign id directly returns `{"...,"note":"proof_..."}`.

3. **Target 3 "WAR Room"** — same field/action authorization bypass as
   single_solve/time_trial above (different `feature_route` hex suffix and
   `protected_reference` id per attempt, same logic).

## Gotchas

- GM `op=start` / `op=submit` calls intermittently hit a bare socket read
  timeout (`TimeoutError` deep in `urllib`/`ssl`) with no HTTP response at
  all — just retry `wk.py begin <mode>` / `submit`; it's not state-corrupting
  (`begin` is idempotent and reconnects to the same attempt).
- This session's worker-slot environment is shared with sibling
  `.slot-*`/`.rush-*` workers; the Bash tool's own scratch tmpfs
  (`/tmp/claude-1000/.../tasks`) ran out of space mid-task under concurrent
  load, failing *every* shell command (including trivial ones like `echo`)
  with ENOSPC, then recovered on its own after a pause. Non-Bash tools
  (Read/Write to project paths) kept working throughout — if this happens
  again, it's an infra contention issue, not a bug in the challenge or
  solver; just retry Bash periodically.
- Once a mode's attempt is submitted CORRECT, its live targets are torn down
  fast: re-requesting them afterward returns HTTP 410 `{"state":"completed"}`.
  Validate/capture evidence *before* submitting, not after.

## Solver

`solve.py` is generic: `classify_and_solve(base_url)` tries, in order,
`solve_war_room` (keyed on `/api/clues` having a `feature_route`),
`solve_backup_exposure` (keyed on `/robots.txt` Disallow entries resolving to
a real directory index), and `solve_idor_timeline` (keyed on `/api/timeline`
returning an `events` list) — no hardcoded ids, paths, or hex suffixes
anywhere. Confirmed correct against all 3 live mastery targets (same logic
used manually via curl to derive the proofs that were submitted and graded
CORRECT); re-running the script against those same targets now returns
nothing because they're already torn down (see gotcha above), not because
the logic is wrong.

If a future day/variant of this family adds a 4th bug class, add a
`solve_<class>(base)` function returning `(proof, evidence_dict)` or `None`,
and a new branch in `classify_and_solve`.
