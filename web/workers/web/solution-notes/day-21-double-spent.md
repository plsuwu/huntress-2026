---
day: 21
slug: "double-spent"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: []  # TODO: fill in descriptive kebab-case bug-class/technique tags, e.g. [reflected-xss, idor]
---

# web-exploitation-day-21-double-spent (release day 27)

All three modes CORRECT: `single_solve`, `time_trial`, `mastery`.

Curriculum day 21 (release day 27).

## New bug class: credit double-spend via a non-atomic check-then-act race

App "Receipt..." (**Rails**). Prompt: *"One credit, two purchases. The
ledger can't count what it can't see coming — race it and spend the same
credit twice."*

`GET /api/clues` discloses:

```json
{"framework":"Rails","feature_route":"/execute-<hex>","secondary_route":"/audit-<hex>","current_role":"analyst"}
```

`feature_route` (POST) redeems one credit: reads the stored count, checks
it is `> 0`, then writes `count - 1` back as two separate steps with no row
lock / atomic `UPDATE ... SET credits = credits - 1 WHERE credits > 0`.
`secondary_route` (GET) reports the proof once the stored balance has gone
negative (`{"error":"credit_not_overspent","credits":0}` until then).

### The exploit

A single, *serial* POST to `feature_route` behaves "correctly"
(`credits: 1 -> 0`, `{"ok":true,"effect":"credit_redeemed"}`) — it does
**not** exhibit the bug by itself, and it **permanently burns the only
credit this target will ever have** (see Gotcha below). The bug only shows
when the account still holds exactly one credit and two or more requests
hit `feature_route` at the same wall-clock instant: every thread that reads
`credits=1` before any of them commits its decrement reports `ok:true`,
and the final stored balance goes to `-1` (or lower, with more racers).
`secondary_route` then reports:

```json
{"ok":true,"effect":"single_credit_spent_twice","evidence":"proof_<hex>"}
```

`rec_double_spend` in `solve.py`: fire N concurrent POSTs to `feature_route`
via a `ThreadPoolExecutor` (12–16 workers was consistently enough — ~5/16
reported `ok:true` on each of the three confirmed targets below), then GET
`secondary_route` once and extract the proof.

## Gotcha — this is **strictly one-shot per target**, and it is the only
## credit the target will ever get

There is no reset/topup/refund route in the app itself. Once the lone
credit has been spent down to 0 — by **any** serial request, including a
single curl GET/POST made while poking around to understand the shape of
`/api/clues` — there is **no in-app way to replenish it**, and because this
family's `single_solve` and the *untimed* `time_trial`/`mastery` pools are
each effectively a single live target with no `retry`/`practice` available
(`wk.py` has no code path for either, by design — see `docs/policies.md`),
a burned credit stalls the whole family with nothing left to submit.

**Do not touch `feature_route` at all until the race is ready to fire.**
`GET /api/clues` and `GET secondary_route` are safe (read-only, don't touch
the credit); the first touch of `feature_route` must be the concurrent
burst, not a solo "let's see what this returns" request.

### Recovery when the credit has already been burned serially

This happened once on this attempt's `single_solve` target (burned by an
earlier/different session before this one started working it — `/execute-*`
returned `409 {"ok":false,"credits":0,"effect":"credit_unavailable"}` on
every racer, `secondary_route` stayed at `credits:0`). `wk.py` intentionally
has no `reset`/`retry` path (worker policy: "No worker may reset/retry/
switch targets" — `docs/policies.md`), so recovering this is **not** a
worker-level action.

The GM does expose an **undocumented per-challenge `op=reset`** (not in
`AGENTS.md`'s operations table, not implemented anywhere in `wk.py`/
`web_rush.py` as of this writing) that tears down the live target and
re-provisions a fresh one *on the same attempt*, resetting in-app state
(credits back to 1, new route suffixes):

```
POST / {"op":"reset","challenge":"<id-or-id/mode>"}
-> {"result":"reset","challenge":"...","outcome":"fresh_target","next":"start",
    "message":"Your target was torn down and a fresh one will be prepared on
    this same attempt. Press Start."}
```

Follow with a normal `op=start` (`wk.py begin <mode>` is fine — it's a
plain idempotent `start`) to get the new target URL, then fire the race
immediately as the very first touch of `feature_route`. This was used once,
at the user's explicit direction, to recover this exact family's stalled
`single_solve` attempt on 2026-10-27 — it is **not** part of the standing
worker toolchain and should not be called from `wk.py` or treated as a
routine recovery path; only use it when a human explicitly directs it (the
standing policy against workers calling reset/retry stands otherwise).

## Graded

- **single_solve**: CORRECT - `proof_d3b4e113009d4d6ca676ce926f2b96fd`
  (feature_route `/execute-bdb3f6`, secondary_route `/audit-ad790f`; 5/16
  racers `ok:true`). Recovered after an `op=reset` (see Gotcha above) — the
  original target's credit had already been burned serially before this
  session reached it.
- **time_trial**: CORRECT - `proof_efc7a04cc5234c45b3e0ea4784845793`
  (feature_route `/dispatch-2bcb70`, secondary_route `/status-e44c77`; 5/16
  racers `ok:true`). Untimed/regular (no `run_for_time`; `wk.py` has no
  timed path by design). Fresh target, never touched before the race —
  worked first try, no reset needed.
- **mastery**: CORRECT - 4 targets, 4 independently-classified bug classes,
  all recovered on the first pass, submitted together.

## Mastery: 4 targets - cumulative, matches the standing shape exactly

The usual backup + IDOR + day-19 group-closing SQLi + this family's own
class — no surprises, join `questions[].artifact` -> `targets[].id`:

1. **Target 1**, **backup exposure** -> `proof_9032b73e9c3749acba0f02647dabbcfa`.
   Same as `recovery_lib.rec_backup`.
2. **Target 2**, **IDOR** -> `proof_e1fc4e2760e64b91849bc36837ee8263`.
   Same as `recovery_lib.rec_idor`.
3. **Target 3**, **SQL injection (group-closing boolean)** ->
   `proof_7ad11c49d11b4f2c882049193de6e1fe`. Same as
   `recovery_lib.rec_sqli_group_close`.
4. **Target 4**, this family's own **credit double-spend (race)** ->
   `proof_1a34b26b8f1048c6b6b7f9ae62c885b4`. feature_route `/dispatch-*`
   (per-target hex, not recorded), 5+/16 racers `ok:true` on the first
   burst - never touched serially first, so no reset was needed for this
   one.

All 4 recovered independently, submitted together -> CORRECT on first submit.

## Reusable solver

- `solve.py <base_url> [evidence_dir] [--racers N]` - this family's own
  class only: `GET /api/clues` -> `feature_route`/`secondary_route`, fires
  `--racers` (default 12; 16 used here) concurrent POSTs to `feature_route`
  via `ThreadPoolExecutor`, then `GET secondary_route` once and extracts
  `proof_<hex>`. Stdlib only. Exposes `rec_double_spend(t, clues, racers=N)`
  for reuse and `Target`/`PROOF_RE` for `mastery.py` to import.
- `mastery.py <evidence_root> <base_url_1> [...]` - per-target classifier.
  Tries this family's own shape first (`feature_route` + `secondary_route`
  with none of `inventory_route`/`protected_reference`/`event_type`/
  `state_format_route`/`session_hint`/`callback_nonce` alongside — that
  exact clue shape is otherwise claimed by `recovery_lib.rec_ssrf`, so SSRF
  is tried as a fallback if the race finds no proof on that shape), then
  falls through to `recovery_lib`'s IDOR / SQLi / backup-exposure in
  evidence-led order, matching `recovery_lib.classify_and_recover`'s
  convention for every other family's mastery.

## For the coordinator - new recoverer to fold into `recovery_lib.py`

`rec_double_spend(t, clues, racers=12)` (ported from `solve.py` in this
directory, stdlib only — no new dependency). Dispatch signature:
`clues.get("feature_route") and clues.get("secondary_route")` **with none
of** `inventory_route`/`protected_reference`/`event_type`/
`state_format_route`/`session_hint`/`callback_nonce` present (shares its
bare two-key shape with `rec_ssrf` — try this family's class first, SSRF
as the fallback on that exact shape, same order `mastery.py` already uses).
Logic: `GET /api/clues`, fire a thread-pool burst of concurrent POSTs at
`feature_route` (never a solo probe first - see Gotcha), `GET
secondary_route` once, extract `proof_<hex>`.

Also worth folding in: the undocumented `op=reset` GM call (see Gotcha
above) as a **coordinator-only**, explicitly-user-directed recovery path
for a family whose lone untimed credit got burned by an earlier pass —
never auto-triggered, never exposed to `wk.py`/workers.

## Gotchas

- **One-shot bug, permanent burn.** Touching `feature_route` even once,
  serially, for any reason (including idle recon) spends the only credit
  this target will ever have, with no in-app recovery. Always do
  `GET /api/clues` + `GET secondary_route` for recon; never a solo
  `POST feature_route`.
- **The race must be the very first interaction with `feature_route`.**
  A concurrent burst against an already-spent target just returns
  `409 {"ok":false,"credits":0,"effect":"credit_unavailable"}` on every
  racer — that response means the credit is already gone, not that the
  race technique failed.
- **`op=reset` exists but is explicitly outside the worker toolchain.**
  `wk.py`/`web_rush.py` have no code path for it and the standing policy
  (`docs/policies.md`) forbids workers from resetting/retrying targets.
  Only use it when a human explicitly asks, and only as a one-off recovery
  for a specific stuck attempt — do not wire it into any automated pass.
