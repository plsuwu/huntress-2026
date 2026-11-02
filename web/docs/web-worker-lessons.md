# Web worker lessons from restored solutions

These are historical, graded observations from the release Day 19-29 notes and
source in `solver_py_scripts/` and `workers/web/past-solutions/`. They are
reference methods, never current proofs, target assignments or permission grants.
Read this before investigating a new target. Do not infer that a new challenge
uses an earlier bug.

## Mastery is cumulative

**Classify every target independently.** The restored Day 19 notes initially
assumed mastery repeated only the named challenge's bug; their later graded
record explicitly corrects that assumption. IDOR mastery mixed backup exposure
and IDOR; SQLi mastery mixed backup exposure and SQLi. Day 20 mastery mixed
backup exposure with LFI or Django authorization bypass. Day 21 mastery mixed
backup exposure (and, for session-impression, IDOR too) with that family's own
reflected-XSS-callback, stored-XSS or session-fixation bug. Day 22 mastery
mixed backup exposure **and** IDOR (a day-19/day-21 class) with that family's
own CSRF or upload-bypass bug — mastery pulls from the whole curriculum
history, not just the current release day. A worker owning one challenge
family therefore needs earlier families' methods for its mastery. Day 24
mastery (two families, three/four targets each) stayed consistent with this:
`the-api-has-opinions` mixed backup exposure and IDOR with that family's own
field/action authorization bypass; `fetch-quest` mixed backup exposure, IDOR
**and** day-19's group-closing SQLi (the exact `') OR ('1'='1` payload, reused
verbatim — comment-based and plain `' OR '1'='1` payloads did not work against
that app) with that family's own SSRF. Day 25 (three families, 4 targets each,
12 total) stayed exactly on this same shape: **every** family's mastery was
backup exposure + IDOR + day-19's group-closing SQLi + that family's own novel
bug (command injection / reset-token confusion / GraphQL field bypass
respectively) — the same three recurring classes plus one new one, every
single time, across three independent families on the same release night. That
is strong enough evidence to treat backup exposure + IDOR + group-closing SQLi
as the *expected* first three mastery targets for any future family, with the
fourth being whatever that family's own class turns out to be.
Day 27 (three families, release day 27) extended this again: `double-spent`
stayed at the standard 4 targets (backup + IDOR + group-closing SQLi + that
family's own race); `socket-to-me` and `forwarded-misgivings` both saw **5**
targets for the first time — the same three recurring classes **plus day-20's
LFI** (newly appearing in this slot's cumulative pool) **plus** that family's
own novel class, one target per class, no repeats. Do not hard-code an
expected count of 4 — always read the actual `questions`/`targets` arrays; a
family's cumulative pool can grow as the event's curriculum does. Day 28
(two families, release day 28; curriculum days 24-25) confirmed the new
5-target shape on both families: backup exposure + IDOR + day-19's
group-closing SQLi + day-20's LFI + that family's own novel class
(fail-open policy decoding / missing audit-log event), one target per class,
no repeats. Day 29 (two families; curriculum days 26-27) again confirmed the
same five-target shape with unverified-update tampering and SSRF/internal-IDOR
as the respective novel classes.

Join `questions[].artifact` to `targets[].id` (for example `Target 2`) to find
the correct URL. Do not rely on array order, curriculum number or title. Preserve
question IDs and the exact `answer_ids` in start.json; solve every assigned
target. Earlier runs often had two mastery targets, but count the current
assignment rather than hard-coding two.

The historical finding to submit was the complete `proof_<hex>` string recovered
from the assigned target, trimmed as requested. It is distinct from the flag
returned by the GM after grading. Never reuse saved proofs, flags, cookies or
target URLs. The current question's answer format remains authoritative.

## Recognizing earlier target families

Use these as evidence to investigate, not a classifier that forces every new
page into one of the categories below. In particular, **a form alone does not
prove SQLi**, and an unknown page does not establish backup exposure. New web
classes must remain unknown until supported by the page, behavior and question.

| Family | Historical evidence and method | Source reference |
|---|---|---|
| Backup exposure | Root comment hints at crawler policy → robots.txt Disallow discloses a directory → open directory listing names a backup → inspect that named file. Derive directories/files from current responses; no blind wordlists. | `workers/web/past-solutions/day-01-backup-exposure-solve.py` |
| IDOR | Root establishes a session and links an activity feed. A case page's inline fetch shows the object API template. Feed entries disclose a foreign object ID/owner; request that evidence-derived ID using the same cookie jar. Paths such as briefs/incidents/history/events vary. | `workers/web/past-solutions/day-02-idor-solve.py` |
| SQLi | Derive the search form action and input name. The earlier query combined visibility with a parenthesized LIKE expression; a group-closing boolean condition revealed the hidden proof row. Comment variants sometimes failed and errors were hidden as “Nothing matched.” Use a small evidence-led payload set, not column sweeps or arbitrary fuzzing. | `workers/web/past-solutions/day-03-sqli-solve.py` |
| LFI | PHP viewer links disclose the endpoint and parameter; the prompt names the mounted objective file. Viewer names and directory depth vary. Earlier code tried bounded traversal depths 1–8, stopping at the first proof, then one absolute-path fallback. Derive the objective path from the current prompt. | `workers/web/past-solutions/day-04-05-shared-common.py`, `rec_lfi` |
| Django authorization bypass | An explicitly linked clues API reports framework/current role and a randomized feature_route. In the historical app the unslashed feature route bypassed the per-route check and returned proof in evidence; the slash variant failed. Derive the route from clues, not a saved path. | `workers/web/past-solutions/day-04-05-shared-common.py`, `rec_admin`; `workers/web/past-solutions/day-05-administratively-speaking-single-solve-focused.py` |
| Reflected XSS + isolated browser callback | `/api/clues` gives feature_route/secondary_route/callback_nonce. feature_route reflects a query param verbatim (no escaping); an isolated browser visiting `<img src=x onerror=fetch(secondary_route?nonce=...)>` would fire that fetch with real browser trust signals. Replay that same request (browser UA + a Referer on feature_route) against secondary_route to collect the proof. | `recovery_lib.py`, `rec_reflect_callback`; `workers/web/solution-notes/day-06-reflect-upon-this.md (also day-07/day-08)` |
| Stored XSS (moderator bot) | `/api/clues` gives feature_route (a guestbook), a 403-gated moderator_data_route, and a receipt_route_template. Post a payload bound to the next sequential id that fetches moderator_data_route same-origin (riding the bot's own session) and POSTs the result to that id's receipt route; poll the message page for the receipt block. | `recovery_lib.py`, `rec_stored_xss_bot`; `workers/web/solution-notes/day-06-reflect-upon-this.md (also day-07/day-08)` |
| Session fixation | `/api/clues` gives feature_route/secondary_route/session_hint. Planting `?sid=<session_hint>` on feature_route makes the app adopt the client-chosen id as its own session cookie; replaying that same id on secondary_route returns the proof. | `recovery_lib.py`, `rec_session_fixation`; `workers/web/solution-notes/day-06-reflect-upon-this.md (also day-07/day-08)` |
| CSRF (state-changing GET) | `/api/clues` gives feature_route; the real state change is a plain GET with a cosmetic, never-checked csrf_token. Forging that GET with cross-site Origin/Referer/Sec-Fetch-Site headers while riding the already-established session flips the server's own cross-site-change flag and releases the proof. | `recovery_lib.py`, `rec_csrf`; `workers/web/solution-notes/day-09-forged-with-care.md (also day-10)` |
| Upload content-type/extension bypass | `/api/clues` gives feature_route (upload) and render_route (preview). The stated extension/content-type policy is never enforced server-side; uploading a filename with a different extension (e.g. `.html`) routes to a preview path that evaluates a template placeholder, disclosing the proof. | `recovery_lib.py`, `rec_upload_bypass`; `workers/web/solution-notes/day-09-forged-with-care.md (also day-10)` |
| Second-order SQLi (blind, no oracle) | `/api/clues` gives feature_route (POST; accepts any JSON object body, always `201`) and secondary_route (GET; a static-looking `{"rows":[...]}` body). The POST field that matters is not a content word (`question`/`note`/`text`, ...) but an SQL-clause-themed name (here `filter`); the payload must actually break out of quoting (`' OR 1=1-- `), not just submit an inert placeholder string like `"1=1"`. There is no error, reflection or timing signal distinguishing a correct field+payload pair from an incorrect one — only the GET route's row count changes once the right combination is stored and re-queried. | `day-12-ask-again-later/single_solve/NOTES.md` |
| SSRF via a fetch proxy | `/api/clues` gives feature_route (fetches an absolute `url` query param server-side) and secondary_route (an "internal_only" route, gated by source IP, that refuses direct external requests). Pointing feature_route at `http://127.0.0.1:<port><secondary_route>` makes the fetch originate from the server's own loopback, satisfying the internal-only check. Only the internal port is unknown; a short port list (8080 confirmed every time so far) finds it fast. | `recovery_lib.py`, `rec_ssrf`; `workers/web/solution-notes/day-15-fetch-quest.md` |
| Field/action authorization bypass | `/api/clues` gives feature_route + inventory_route + protected_reference (a generic "execute" endpoint). `inventory_route` discloses the exact fields/actions/query-parameter-name grid — no guessing needed. Authorization is checked per `(field, action)` pair inconsistently: one action (seen: `export`) is not gated the same way as another (seen: `view`) for the restricted field, and leaks it. Brute-force the small disclosed grid, not a wordlist. | `recovery_lib.py`, `rec_field_action_bypass`; `workers/web/solution-notes/day-14-the-api-has-opinions.md` |
| OS command injection (unescaped shell path) | `/api/clues` gives feature_route + input_parameter + example_path (a Gin app that shells out, unescaped, to build a directory listing from the raw parameter value). Appending `; <cmd>` to the value runs `<cmd>` as a second shell statement (plain `;`-chaining only — `$(...)` substitution is rejected server-side with `diagnostic_failed` on every target seen). The proof is never reflected directly; it has to be `cat`-ed from the file named by the container's own `OBJECTIVE_PATH` env var (itself only visible via an `env` injection). | `recovery_lib.py`, `rec_command_injection`; `workers/web/solution-notes/day-16-shell-company.md` |
| Password-reset token / account-id confusion | `/api/clues` gives feature_route (request a token) + secondary_route (redeem it) + recovery_guide_route (discloses account_parameter/token_parameter names and the self_service-flagged account ids — no guessing needed). feature_route refuses to issue a token for a non-self-service account, but secondary_route never binds the redeemed token to the account_id it is redeemed against: request a token for a self-service account, then redeem it while claiming a different, non-self-service account id. | `recovery_lib.py`, `rec_reset_account_confusion`; `workers/web/solution-notes/day-17-reset-expectations.md` |
| GraphQL field-level authorization bypass | `/api/clues` gives feature_route (a live GraphQL endpoint, POST `{"query": ...}`) + protected_reference (an id hidden from a list query's sensitive field). Authorization is enforced by omission on the list surface (e.g. `caseIndex`, which never returns the sensitive field at all) but the matching single-object surface (e.g. `case(id)`) has no field-level check whatsoever — query it directly for `protected_reference`. "Vary the protocol surface" means switch GraphQL *operation* (list → single-object), not HTTP method. Needs a browser-like User-Agent — a bare urllib UA gets 403'd by the edge WAF on this app. | `recovery_lib.py`, `rec_graphql_field_bypass`; `workers/web/solution-notes/day-18-resolver-revolt.md` |
| Forged/unsigned webhook event trust | `/api/clues` gives feature_route + event_type + owner_scope (+ current_role) — no `secondary_route`/`inventory_route`/`protected_reference`/`state_format_route`, so it's distinguishable from every class above that also starts from feature_route. The receiver at feature_route has **no signature/HMAC/auth check at all**: a GET returns `post_required`, an empty POST returns `webhook_rejected`, but it blindly trusts any POST body that claims the disclosed event_type/owner_scope. The exact field names are *not* disclosed and are easy to over-guess: the receiver wants exactly two bare keys, `{"event": event_type, "owner": owner_scope}` — not the clue's own key names (`event_type`/`owner_scope`), not `type`/`scope`, no nesting under `data`/`payload`. Both keys required, both values matched exactly. The check is stateless (identical proof on every correct repeat), so probing field-name/value variants directly against a live target is safe. | `recovery_lib.py`, `rec_forged_webhook_event`; `workers/web/solution-notes/day-19-trust-the-webhook.md` |
| Credit double-spend (non-atomic check-then-act race) | `/api/clues` gives feature_route (POST; redeems one credit — reads the stored count, checks `> 0`, writes `count-1` back as two separate steps, no row lock/atomic UPDATE) + secondary_route (GET; reports the proof once the stored balance has gone negative) — the same bare two-key shape as SSRF above, with no clue field to tell them apart. A single serial POST behaves "correctly" and **permanently burns the target's one and only credit** (no in-app reset/topup); the bug only shows when ≥2 requests hit feature_route at the same wall-clock instant, so the race must be the very first touch of feature_route, never a solo probe. 12–16 concurrent racers via a thread pool was consistently enough (~5/16 `ok:true`). | `recovery_lib.py`, `rec_double_spend`; `workers/web/solution-notes/day-21-double-spent.md` |
| Unauthorized event-stream subscription (WS upgrade, wrong query-param *name*) | `/api/clues` gives feature_route (a real WebSocket upgrade path, not a normal route — plain GET/POST without `Upgrade` headers returns 426) + stream_reference (a numeric id), no other feature_route-shaped key alongside. A well-formed WS handshake with no query, or with the clue's own key name (`stream_reference`/`ref`/`id`/`channel`), gets a 403 that *looks* like a role/auth rejection but isn't — the upgrade only succeeds when the query string names the stream with the bare, undisclosed key `stream`. Once connected, the server immediately pushes one proof-carrying event, never re-checking the caller's role/ownership of that stream. Needs the non-stdlib `websockets`/`certifi` packages (stdlib `urllib` can't do a clean WS handshake). | `recovery_lib_ws.py`, `rec_unauthorized_event_stream`; `workers/web/solution-notes/day-22-socket-to-me.md` |
| Trusted-reverse-proxy header spoofing | `/api/clues` gives feature_route + current_role + `deployment: "trusted_reverse_proxy"` — no `secondary_route`/`inventory_route`/`protected_reference`, distinguishable by that key shape alone. feature_route decides "am I being called from loopback" from a client-supplied `X-Forwarded-For` header with **no actual upstream-proxy allowlist**: sending `X-Forwarded-For: 127.0.0.1` flips `evaluated_client` and returns the proof. `X-Real-IP`/`X-Forwarded-Host`/`X-Forwarded-Proto` do not affect the decision — XFF alone is necessary and sufficient. Stateless; safe to re-probe. | `recovery_lib.py`, `rec_proxy_trust_bypass`; `workers/web/solution-notes/day-23-forwarded-misgivings.md` |
| Fail-open remote policy decoder | `/api/clues` gives feature_route + `policy_format_route` + `expected_behavior` (seen: ASP.NET Core Razor Pages). `policy_format_route` discloses the exact query-parameter name, per-attempt `remote:<hex>:` prefix, encoding (base64url UTF-8 JSON), and a valid public decision. A valid public decision only returns the standard policy response, but a token with the disclosed prefix plus an **undecodable payload** hits the decoder's exception path and grants the protected effect, returning the proof — the opposite of the disclosed expected failure-deny behavior. Stateless; derive the parameter and prefix live. | `recovery_lib.py`, `rec_fail_open_policy`; `workers/web/solution-notes/day-28-fail-open-friday.md` |
| Missing audit-log event on a privileged action | `/api/clues` gives the same bare feature_route + secondary_route shape as SSRF/double-spend, but a read-only GET on secondary_route discloses `report.fields` (seen: Slim/PHP). POST feature_route performs a privileged export and returns an id/operation; GET secondary_route shows only a benign event for that id. POST secondary_route with the disclosed report body (`export_id`, `missing_event`, `observed_event`) returns the proof. Always GET secondary_route first: it separates this class from SSRF (403) and from double-spend (whose feature_route is one-shot and must never be probed serially). | `recovery_lib.py`, `rec_missing_log_event`; `workers/web/solution-notes/day-28-if-a-breach-falls-in-a-log.md` |
| Unverified dependency update / hook tampering | `/api/clues` gives `feature_route`, `update_feed`, and `update_policy`. Copy a feed update, change only `artifact.hook` to a policy-declared restricted hook, and retain its original signed hash/signature. The updater activates the tampered artifact without verifying the disclosed integrity contract. Derive the package object and hook live. | `recovery_lib.py`, `rec_unverified_update`; `workers/web/solution-notes/day-29-trust-the-build.md` |
| SSRF chained to internal IDOR | `/api/clues` gives `feature_route`, `topology_route`, `case_directory_route`, and `current_role`. Topology discloses an absolute loopback case-service URL; the case directory discloses a foreign id. Pass that internal URL with `id=<foreign id>` through the public fetch parameter (disclosed by the feature route's 400 response). | `recovery_lib.py`, `rec_internal_affairs`; `workers/web/solution-notes/day-29-internal-affairs.md` |

The historical dispatcher checked admin clues **before generic API/IDOR signs**,
then PHP viewer/LFI evidence, SQLi, IDOR, and finally investigated backup hints.
Both admin and IDOR expose `/api/` links, so testing generic API signs first
misclassifies the admin app. `recovery_lib.classify_and_recover` extends that
same discipline for the eight classes above: it always attempts `GET /api/clues`
directly rather than gating on the root page literally containing that text (the
day-07 app only discloses it via `/openapi.json`, never a root-page link — a
text pre-filter silently misses it), tries each clues-driven class in turn (now
including command injection, reset-token confusion, SSRF, field/action bypass
and GraphQL field bypass — each distinguished by its own exact clue-key
combination, since several share `feature_route`/`secondary_route` and only
differ by which *other* key is or isn't present), then IDOR, then a
`<form>`-based SQLi gate, then backup exposure last (with a `<form>`-present
SQLi retry and a backup-exposure retry as final fallbacks). Extend this evidence hierarchy for
new classes; do not blindly copy its form=SQLi or fallback=backup assumptions.
Day 28 and Day 29's classes are now folded into `recovery_lib.py`. For the ambiguous bare
`feature_route` + `secondary_route` shape, the canonical dispatcher makes one read-only GET on
`secondary_route`: `report.fields` selects missing-audit-log, a `credits` balance selects the
one-shot double-spend race, and 403/`internal_service_only` selects SSRF. An unrecognized response
fails closed for worker investigation rather than speculatively firing a state-changing exploit.

When a target gives no error, reflection or timing oracle at all (day-12's
second-order SQLi above), a one-variable-at-a-time sweep can look exhaustive
while still missing the answer: testing candidate field names only against an
inert placeholder value, and testing real quote-breaking payloads only against
content-word field names, never tests the cell that needs both right at once.
Day-12's first pass ran ~100+ requests and concluded "blocked" this way; the
fix was to run field name and payload shape as a deliberate cross-product
(a small, evidence-derived matrix — SQL-clause-themed names like `filter`,
`where`, `sql` crossed with real boolean-OR/UNION payloads — not a blind
wordlist). Before recording a target as blocked for lack of an oracle, confirm
every evidence-backed candidate on one axis was actually tried combined with
every plausible value on the other axis, not just independently.

### A working hit is not the same as knowing the mechanism

Day 26's trust-the-webhook (above) showed the opposite failure mode from
day-12's: a first-contact agent met a brand-new, undocumented bug class (no
table row matched its clue shape) and, under "speed first" pressure with a
free-retry untimed attempt, combined every plausible field-name alias
(`event`/`type`/`event_type`, `owner_scope`/`scope`/`owner`) into one POST
body, flat and nested under `data`/`payload` at once. It got a `200` on the
first such attempt — a real win for `single_solve` — but could not say which
field or nesting actually mattered. That is not the same thing as having
found the mechanism, and a kitchen-sink body of unknown composition cannot be
folded into `recovery_lib.py` (every `rec_*` there is one tight,
single-insight trigger + minimal payload) or trusted to transfer to another
target's field names.

**Do not stop at a first hit that merges many unverified guesses into one
request.** Before writing a new class up as *the* method: check whether
repeating the exact same working request returns the exact same proof. If it
does, the check is stateless (not a one-shot consumable event), and it is
safe to binary-search the kitchen-sink body down to its minimal form directly
against the live target — halve the fields, see if it still works, bisect
from there. On trust-the-webhook this took six follow-up requests to go from
a 9-key body to the true minimal payload (two bare keys, neither of which was
the clue's own field name — see the table row above). Do not confuse this
with the day-12 cross-product advice just above: that is about running a
*matrix of separate, individually-interpretable requests* across two
genuinely uncertain axes when there is no oracle at all; it is not license to
merge every alias into a single body and declare victory once one combination
sticks.

### A bare 403 on a protocol upgrade is not necessarily a role/auth check

Day 27's socket-to-me burned real investigation time assuming a 403 on a
well-formed WebSocket handshake was a role-elevation puzzle — tried every
role name as a header/query/subprotocol value, tried Cloudflare-bot-check
workarounds with full browser fingerprint fidelity across three different
WS clients, all got the identical 403. The 403 was never about *who* you
are; it was about *what query-parameter key* you used to name the one
numeric clue field, and the actual key was undisclosed (not the clue's own
field name). Before concluding a WS-upgrade 403 is an auth/role rejection,
systematically vary the query-parameter *name* (not just its value) for
every disclosed clue field — same cross-product discipline as the day-12
no-oracle lesson above, applied to parameter *names* instead of a payload
matrix.

## HTTP and lifecycle discipline

- Use a separate cookie jar per assigned target. A root GET established the
  session needed by the IDOR object requests. Preserve same-origin redirects
  when needed (the PHP app redirected root to its viewer), but refuse redirects
  outside the assigned origin. Do not forward GM authentication.
- Preserve root pages, response headers, status codes and the responses that
  support each finding. Use unique target/attempt evidence directories; do not
  mix a prior variant's dumps into a current result or delete the old record.
- Historical clients paced requests about 0.7–0.8 seconds apart. Keep bounded
  request counts and finite, generous timeouts; recorded GM calls used 30–45
  seconds. A timeout or 5xx is evidence, not permission to loop or change mode.
- `result="preparing"` with no targets was observed before `result="active"`.
  Waiting and status polling belong to `tools/wk.py begin`. A historical
  `503 mode_configuration_invalid` blocked mastery until the service was fixed;
  it was a service configuration problem, not an exploit failure. That incident
  was resolved and is not a claim about current service health.
- Historical notes reported empty day/lane-filter results during release.
  Later live metadata in this checkout distinguishes release `day` from
  `curriculum_day`. Discovery belongs to the coordinator; those old “list all”
  suggestions do not authorize workers to query GM or other lanes.
- GM `op=start`/`op=submit` calls intermittently hit a bare socket read
  timeout (`TimeoutError` deep in `urllib`/`ssl`) with no HTTP response at
  all — retry `wk.py begin <mode>`/`submit`; it is not state-corrupting
  (`begin` is idempotent and reconnects to the same attempt). Day 24.
- Once a mode's attempt is submitted CORRECT, its live target is torn down
  fast: re-requesting it afterward returns HTTP 410 `{"state":"completed"}`.
  Capture evidence *before* submitting, not after. Day 24.
- A target you crash or lose mid-attempt (Cloudflare 500 / error 1101, or a
  410 while the attempt is still `active`) is recovered first by re-running
  `python3 tools/wk.py begin <mode>` - NOT by sitting and polling the dead URL
  for an auto-restart. `begin` always re-POSTs `op=start`: on an active untimed
  attempt that re-provisions a fresh target under the same attempt_id (the GM
  rotates the target host); if the old attempt expired it transparently returns
  a new one and warns you, so re-derive any stale answers. If `begin` cannot
  recover it (still 500/410, or a one-shot state was burned), use the scoped
  worker recovery `python3 tools/wk.py reset <mode>` once, then run `begin`
  again and re-derive every answer from the fresh target(s). Better: don't
  crash it in the first place - a malformed/non-object body can trip the origin
  into a 500 and take the container down for the whole attempt, so probe with
  well-formed requests. Day 26 (trust-the-webhook); reset path added Day 28.
- A bare `Python-urllib/*` default User-Agent gets 403'd outright by the edge
  WAF/Cloudflare on at least one app (resolver-revolt's GraphQL endpoint,
  day 25) with no body hint that it's a UA issue — every request, including
  harmless probes, needs a browser-like `User-Agent` header. `recovery_lib.py`'s
  `Target` client already sends one by default; a hand-rolled request client
  must not drop it.
- Some injection sinks reject command-substitution forms (`$(...)`) outright
  with a generic server-side error (`{"error":"diagnostic_failed"}`, day 25's
  shell-company) while accepting plain `;`-chained commands — if a shell
  injection is suspected but `$()` gets rejected, retry with `;` before
  concluding the sink isn't injectable.
- A one-shot bug (day 27's double-spend: a single credit with no in-app
  reset/topup) is permanently spent by **any** serial touch of the triggering
  route, including idle recon made "just to see the shape of the response."
  Once a target's own README/clues disclose that a route is one-shot/stateful
  and consumable, do read-only recon via every other route first and make the
  real attempt (e.g. the concurrent race burst) the very first touch of that
  specific route — never a solo probe "to see what it returns."
- The GM exposes an **undocumented per-challenge `op=reset`**
  (`POST / {"op":"reset","challenge":"<id-or-id/mode>"}`) that tears down a
  live target and re-provisions a fresh one on the same attempt, resetting
  in-app state. Workers may use it only through
  `python3 tools/wk.py reset <mode>` (own family/mode only, bounded, never on
  an already-CORRECT route), then must run `python3 tools/wk.py begin <mode>`
  and re-derive every answer from the fresh target(s). Direct curl/raw reset
  remains a coordinator-only, explicitly-user-directed recovery for cases the
  worker guard cannot handle. Day 27 (first coordinator use); Day 28 (wired
  into `wk.py` after repeated target-recovery friction).
- `targets[].url` ends with a trailing slash. Strip it (`url.rstrip('/')`)
  before concatenating any route. A double slash (`//cases/1`) is not harmless:
  it 404s on the IDOR app, falls through to a catch-all fallback on the backup
  app (making the real `/robots.txt` look missing), and reaches a broken
  handler on the PHP LFI app that mimics a parameter-parsing bug. If every
  route except `/` 404s, check for a double slash before deeper diagnosis.
  `recovery_lib.Target` already strips it; hand-rolled clients must too. Day 28.
- A different 404 size under `Accept: text/html` can be a Cloudflare-insights
  rendering artifact, not routing evidence; an app's root `/robots.txt` can be
  Cloudflare's boilerplate content-signals file with no `Disallow`. Proofs are
  per-target-instance, so after any reset/re-provision re-derive all answers
  before submitting. Day 28.
- PHP `include()` built from raw string concatenation (`$pages_dir . $doc`,
  no path-join/validation) still only accepts a plain traversal path — a
  `php://filter/...` wrapper scheme prefix just produces the literal invalid
  concatenated path, since the fixed directory prefix always wins. Don't
  waste requests on filter-wrapper tricks against this app shape; plain `../`
  traversal is the only lever. Traversing to a real executable file under the
  same webroot (e.g. `../index.php`) can crash the container (seen: Cloudflare
  error 1101, self-recovered in ~3s) — only traverse to known-plain-text
  targets. Day 27 (forwarded-misgivings).
- `ssl.create_default_context()` on this sandbox's default Python has no
  system CA trust store wired up for `urllib`/`ssl` — for any HTTPS code path
  outside `recovery_lib.py`'s own `Target` (e.g. a hand-rolled WebSocket
  client), pass `cafile=certifi.where()` or export
  `SSL_CERT_FILE=$(python3 -c "import certifi; print(certifi.where())")`, or
  every request fails with `CERTIFICATE_VERIFY_FAILED` before reaching the
  app. Day 27 (socket-to-me).

## Reuse source, not the old execution workflow

The canonical executable module is `solver_py_scripts/recovery_lib.py`: one shared,
stdlib-only module folding every HTTP class above (`rec_backup`, `rec_idor`, `rec_lfi`,
`rec_session_fixation`, `rec_reflect_callback`, `rec_stored_xss_bot`, `rec_csrf`,
`rec_upload_bypass`, `rec_ssrf`, `rec_field_action_bypass`, `rec_command_injection`,
`rec_reset_account_confusion`, `rec_graphql_field_bypass`, `rec_forged_webhook_event`,
`rec_sqli_group_close`, `rec_double_spend`, `rec_proxy_trust_bypass`,
`rec_fail_open_policy`, `rec_missing_log_event`, `rec_unverified_update`, and
`rec_internal_affairs`) plus the `Target` HTTP client and `classify_and_recover`
dispatcher. It is imported directly by the automated first pass and the provisioned
all-mode `solve.py`; workers do not copy these functions into another mastery script.
`rec_sqli_group_close` folds in day-03's repeatedly graded payload, and
`rec_forged_webhook_event` retains the minimal payload established by live minimization.
`solver_py_scripts/recovery_lib_ws.py` is the sibling for the WebSocket class that needs
non-stdlib `websockets`/`certifi`; install those only into an ephemeral venv before
importing it. Offline regressions under `tests/` use mocks/local data and cover the
first-pass lifecycle, latest recoverers, ambiguous clue shapes, and answer-reuse guards.
Graded live evidence remains the basis for promoting a recoverer; a unit test alone does
not establish that a new mechanism works.
Earlier still: `workers/web/past-solutions/day-04-05-shared-common.py` (`sniff`,
`rec_sqli`, `rec_lfi`, `rec_admin`) and `day-01-02-03-shared-solve_lane.py` (the earlier
three-family dispatcher). Every note in `workers/web/solution-notes/` (including the
days-06-through-10 "later consolidated reference" sections) includes dated corrections
and graded evidence; read the final findings before adopting an earlier assumption.

Read `workers/web/past-solutions/*.py` and `workers/web/solution-notes/*.md` as source
only. **Do not execute or import anything in `past-solutions/`:** these are kept verbatim
as a historical/reference record, not meant to be re-run from their flattened location -
several made GM calls, started/submitted attempts, exposed timed/retry switches,
sometimes read credentials at import, or wrote shared run directories when they lived in
their original per-challenge directories, and some now have broken relative imports
(`../recovery_lib.py`, a sibling `common.py`) since being flattened. The day-01/02/03
scripts need `requests`; most others use stdlib urllib.
Relocated credential/root-path fallbacks are obsolete. Older clients also follow
redirects by default and some recoverers contain guessed fallback paths.
`recovery_lib.py` is deliberately different from those historical scripts: it is the
reviewed canonical module, has no GM calls or credentials, and is intended for direct import
by the current worker starter. Edit only the provisioned solver's `rec_this_family` hook,
then reuse it for every mode. Canonical library changes are coordinator-reviewed and must
pass `python3 -m unittest discover -s tests -v`; workers must not edit the shared library
in place during a challenge. Continue to use current assignments, scoped HTTP requests,
response-derived paths, the coordinator's submission flow, and the untimed-only policy.
