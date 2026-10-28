---
day: 20
slug: "serialized-offender"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [unsigned-state-tampering, privilege-escalation, backup-exposure, idor, sqli, client-side-state, base64url, json-state, role-tampering, role-discovery, missing-signature-validation, boolean-sqli, group-closing, target-classification, target-teardown]
---

# web-exploitation-day-20-serialized-offender (release day 26)

All three modes CORRECT: `single_solve`, `time_trial`, `mastery`.

Curriculum day 20 (release day 26) — confirms the lane-patterns prediction that
release day 26 continues sequentially from curriculum day 19 (this is day 20).

## New bug class: insecure deserialization / unsigned session-state role trust

App "Object Post" / "State Suitcase" (**Quarkus**, Java). The prompt — *"Your
session is just a blob of misplaced trust. Reshape the bytes, sign nothing, and
see what the app suddenly believes about you"* — is literal: the session state
is a **base64url-of-JSON blob with no signature/HMAC**, carried in a POST body
field, and the server trusts the `role` the client writes into it.

`GET /api/clues` discloses:

```json
{"framework":"Quarkus","feature_route":"/process-<hex>",
 "state_format_route":"/api/state-format","current_role":"analyst"}
```

`GET /api/state-format` discloses the exact transport/encoding (no guessing):

```json
{"transport":"POST /process-<hex>","content_type":"application/json",
 "request_field":"state","encoding":"base64url of UTF-8 JSON",
 "sample_state":{"user":"analyst","role":"analyst"},
 "sample_encoded":"eyJ1c2VyIjoiYW5hbHlzdCIsInJvbGUiOiJhbmFseXN0In0"}
```

**The exploit** (one POST):
`POST <feature_route>` body `{"state": base64url(json)}` where the JSON is the
sample state with `role` (and the mirroring `user` field) reshaped to a
**privileged role string**. The server reads the unsigned blob, trusts the
role, and the guarded route returns the proof:

```json
{"ok":true,"effect":"serialized_role_trusted","evidence":"proof_<hex>"}
```

### Gotcha — which role is privileged is *not* disclosed

The disclosed `current_role`/`sample_state` role (`analyst`) is rejected, and so
are the obvious `admin` and `{"user":"admin","role":"admin"}`:

```
{"user":"analyst","role":"analyst"}      -> 403 {"error":"role_not_permitted"}
{"user":"analyst","role":"admin"}        -> 403 {"error":"role_not_permitted"}
{"user":"admin","role":"admin"}          -> 403 {"error":"role_not_permitted"}
{"user":"analyst","role":"administrator"}-> 200 {"ok":true,...,"evidence":"proof_..."}
```

The accepted value on every live variant seen was **`administrator`** (the full
word, not `admin`). The solver sweeps a small evidence-led candidate list
(`administrator` first, then `admin`, `superadmin`, `root`, `owner`, ...),
excluding the sample's own un-privileged role. base64url is **unpadded** here —
strip `=` when encoding.

## Graded

- **single_solve**: CORRECT — `proof_52aaa30a394342d3a2b7f89f4fff3b42`
  (feature_route `/process-e04a42`, role `administrator`).
- **time_trial**: CORRECT — `proof_349b3fc951e549e4923ad029b908504c`. Untimed
  (per pipeline policy; `wk.py` has no `run_for_time` path).
- **mastery**: CORRECT — 4 targets, 4 independently-classified bug classes.

## Mastery: 4 targets, cumulative as expected

Exactly the standing shape (backup + IDOR + day-19 group-closing SQLi + the
family's own class — see `docs/web-worker-lessons.md`). Each classified from its
own live evidence, join `questions[].artifact` -> `targets[].id`:

1. **Target 1 (proof-1) — "Tiny Uptime"** (tiny native server), **backup
   exposure**. `/api/clues` 404. Root comment `<!-- deployment note: remember to
   review crawler policy before launch -->`; `/robots.txt` Disallow -> open
   directory index -> `.bak` JSON with
   `proof_a893306e3e214403ac8df700569ca1eb`. Same as `recovery_lib.rec_backup`.
2. **Target 2 (proof-2) — "Incident Picnic"**, **IDOR**. Session dashboard lists
   own `/tickets/<id>`; `/api/timeline` feed discloses a foreign ticket id;
   `GET /api/tickets/<foreign_id>` same cookie jar ->
   `proof_885fdcd5ae3a4d84bc37c72e99f0f6a3`. Same as `recovery_lib.rec_idor`.
3. **Target 3 (proof-3) — "Signal Shelf"**, **SQL injection (group-closing
   boolean)**. `/explore?q=` form over a quoted LIKE clause. Day-19's verbatim
   `') OR ('1'='1` worked first try -> `proof_78ec3c9f3ca64b59b12714835536cd40`.
   (6th independent confirmation of that exact payload across release days.)
4. **Target 4 (proof-4) — "State Suitcase"**, this family's own **unsigned-state
   deserialization**. feature_route `/action-fcc7c3`, role `administrator` ->
   `proof_26fb054fcbe040d389d1eb368c6da0cd`.

All 4 recovered independently, submitted together -> CORRECT.

## Reusable solver

- `solve.py <base> [evidence_dir]` — this family's own class only: reads
  `/api/clues` + `state_format_route`, reshapes the sample state's role across a
  privileged-role candidate list, POSTs base64url(JSON) to the feature route,
  extracts `proof_<hex>` from the response. Fully derives routes/field/encoding
  from the live target.
- `mastery.py <evidence_root> <base...>` — per-target classifier for mastery.
  Ports `rec_backup`, `rec_idor`, `rec_sqli_group_close` from `recovery_lib.py`
  (stdlib-only, scoped to one origin) and imports this day's deser recoverer.

## For the coordinator — new recoverer to fold into `recovery_lib.py`

`rec_unsigned_state_role(t, clues)` (this family's class), dispatch signature:
`clues.get("feature_route") and clues.get("state_format_route")`. Logic:
1. `GET state_format_route` -> `request_field`, `sample_state`, encoding.
2. For each privileged-role candidate (exclude the sample's own role), set the
   `role` key (and the `user` key if it mirrors role) and POST
   `{request_field: base64url_unpadded(json)}` to `feature_route`.
3. Extract `proof_<hex>` from the response. Accepted role seen: `administrator`.
The `classify_and_recover` dispatcher should try this *early* (it shares
`feature_route` with SSRF/GraphQL/etc. but is uniquely identified by
`state_format_route`).

## Gotchas

- Privileged role is the full word `administrator`, not `admin` — don't stop at
  `admin`/`{"user":"admin"}` (both 403 `role_not_permitted`).
- base64url encoding is unpadded (strip `=`).
- Evidence captured before each submit (mastery targets 410 after CORRECT).
- No WAF UA issue hit, but the solver sends a Chrome UA anyway (Cloudflare edge).
