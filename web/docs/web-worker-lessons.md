# Web worker lessons from restored solutions

These are historical, graded observations from the release Day 19/20/21/22 notes
and source in `solver_py_scripts/`. They are reference methods, never current
proofs, target assignments or permission grants. Read this before investigating
a new target. Do not infer that a new challenge uses an earlier bug.

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
family therefore needs earlier families' methods for its mastery.

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
| Backup exposure | Root comment hints at crawler policy → robots.txt Disallow discloses a directory → open directory listing names a backup → inspect that named file. Derive directories/files from current responses; no blind wordlists. | `day-19-web-exploitation/backup-exposure/solve.py` |
| IDOR | Root establishes a session and links an activity feed. A case page's inline fetch shows the object API template. Feed entries disclose a foreign object ID/owner; request that evidence-derived ID using the same cookie jar. Paths such as briefs/incidents/history/events vary. | `day-19-web-exploitation/idor/solve.py` |
| SQLi | Derive the search form action and input name. The earlier query combined visibility with a parenthesized LIKE expression; a group-closing boolean condition revealed the hidden proof row. Comment variants sometimes failed and errors were hidden as “Nothing matched.” Use a small evidence-led payload set, not column sweeps or arbitrary fuzzing. | `day-19-web-exploitation/sqli/solve.py` |
| LFI | PHP viewer links disclose the endpoint and parameter; the prompt names the mounted objective file. Viewer names and directory depth vary. Earlier code tried bounded traversal depths 1–8, stopping at the first proof, then one absolute-path fallback. Derive the objective path from the current prompt. | `day-20-web-exploitation/common.py`, `rec_lfi` |
| Django authorization bypass | An explicitly linked clues API reports framework/current role and a randomized feature_route. In the historical app the unslashed feature route bypassed the per-route check and returned proof in evidence; the slash variant failed. Derive the route from clues, not a saved path. | `day-20-web-exploitation/common.py`, `rec_admin`; `day-05-web-exploitation/administratively-speaking/solve.py` |
| Reflected XSS + isolated browser callback | `/api/clues` gives feature_route/secondary_route/callback_nonce. feature_route reflects a query param verbatim (no escaping); an isolated browser visiting `<img src=x onerror=fetch(secondary_route?nonce=...)>` would fire that fetch with real browser trust signals. Replay that same request (browser UA + a Referer on feature_route) against secondary_route to collect the proof. | `recovery_lib.py`, `rec_reflect_callback`; `day-21-web-exploitation/NOTES.md` |
| Stored XSS (moderator bot) | `/api/clues` gives feature_route (a guestbook), a 403-gated moderator_data_route, and a receipt_route_template. Post a payload bound to the next sequential id that fetches moderator_data_route same-origin (riding the bot's own session) and POSTs the result to that id's receipt route; poll the message page for the receipt block. | `recovery_lib.py`, `rec_stored_xss_bot`; `day-21-web-exploitation/NOTES.md` |
| Session fixation | `/api/clues` gives feature_route/secondary_route/session_hint. Planting `?sid=<session_hint>` on feature_route makes the app adopt the client-chosen id as its own session cookie; replaying that same id on secondary_route returns the proof. | `recovery_lib.py`, `rec_session_fixation`; `day-21-web-exploitation/NOTES.md` |
| CSRF (state-changing GET) | `/api/clues` gives feature_route; the real state change is a plain GET with a cosmetic, never-checked csrf_token. Forging that GET with cross-site Origin/Referer/Sec-Fetch-Site headers while riding the already-established session flips the server's own cross-site-change flag and releases the proof. | `recovery_lib.py`, `rec_csrf`; `day-22-web-exploitation/NOTES.md` |
| Upload content-type/extension bypass | `/api/clues` gives feature_route (upload) and render_route (preview). The stated extension/content-type policy is never enforced server-side; uploading a filename with a different extension (e.g. `.html`) routes to a preview path that evaluates a template placeholder, disclosing the proof. | `recovery_lib.py`, `rec_upload_bypass`; `day-22-web-exploitation/NOTES.md` |
| Second-order SQLi (blind, no oracle) | `/api/clues` gives feature_route (POST; accepts any JSON object body, always `201`) and secondary_route (GET; a static-looking `{"rows":[...]}` body). The POST field that matters is not a content word (`question`/`note`/`text`, ...) but an SQL-clause-themed name (here `filter`); the payload must actually break out of quoting (`' OR 1=1-- `), not just submit an inert placeholder string like `"1=1"`. There is no error, reflection or timing signal distinguishing a correct field+payload pair from an incorrect one — only the GET route's row count changes once the right combination is stored and re-queried. | `day-12-ask-again-later/single_solve/NOTES.md` |

The historical dispatcher checked admin clues **before generic API/IDOR signs**,
then PHP viewer/LFI evidence, SQLi, IDOR, and finally investigated backup hints.
Both admin and IDOR expose `/api/` links, so testing generic API signs first
misclassifies the admin app. `recovery_lib.classify_and_recover` extends that
same discipline for the five classes above: it always attempts `GET /api/clues`
directly rather than gating on the root page literally containing that text (the
day-07 app only discloses it via `/openapi.json`, never a root-page link — a
text pre-filter silently misses it), tries each clues-driven class in turn, then
IDOR, then backup exposure last. Extend this evidence hierarchy for new classes;
do not blindly copy its form=SQLi or fallback=backup assumptions.

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

## Reuse source, not the old execution workflow

The most complete reference is now `solver_py_scripts/recovery_lib.py`: one
shared, stdlib-only module folding every class above (`rec_backup`, `rec_idor`,
`rec_session_fixation`, `rec_reflect_callback`, `rec_stored_xss_bot`, `rec_csrf`,
`rec_upload_bypass`) plus the `Target` HTTP client and `classify_and_recover`
dispatcher behind it, shared by `day-21-web-exploitation/solve.py` and
`day-22-web-exploitation/solve.py`. It is verified against local HTTP fixtures
for every class it recovers (not just read for plausibility), but a fixture is
not a live target: still read it before trusting it against your assignment.
Earlier still: `day-20-web-exploitation/common.py` (`sniff`, `rec_sqli`, `rec_lfi`,
`rec_admin`) and `day-19-web-exploitation/solve_lane.py` (the earlier
three-family dispatcher). Every `NOTES.md` (including the new day-21/day-22
ones) includes dated corrections and graded evidence; read the final findings
before adopting an earlier assumption.

Read shared `.py` and `.md` files as source only. **Do not execute or import the
legacy solver entrypoints under `day-19-web-exploitation/` and
`day-20-web-exploitation/`:** they make GM calls, start/submit attempts, expose
timed/retry switches, sometimes read credentials at import, and write shared
run directories. Day 19 scripts require `requests`; Day 20 uses stdlib urllib.
Relocated credential/root-path fallbacks are obsolete. Older clients also follow
redirects by default and some recoverers contain guessed fallback paths.
`recovery_lib.py` and the day-21/day-22 `solve.py` files are different in kind
(no GM calls, no credentials, no timed/retry switches — they only ever talk to
one target origin named in a `start.json` you already own) but the same
discipline still applies: port the logic into your current job's authored
solver rather than importing this shared module directly against your live
assignment.

Port only the needed parsing/recovery logic into your current job's authored
solver. Use current assignments, scoped HTTP requests, explicit response-derived
paths, the coordinator's submission flow, and the current untimed-only policy.
Never edit the shared restored reference tree or run a saved challenge script.
