---
day: 30
slug: "framework-roulette"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [auth-bypass, header-spoofing, reverse-proxy-trust, x-forwarded-for, loopback-bypass, vary-header-oracle, role-claim-spoofing, broken-access-control, trust-boundary, aspnet-core, blazor-server, django, backup-exposure, idor, sqli, boolean-sqli, group-closing, lfi, path-traversal, target-classification]
---

# Day 31 - web-exploitation-day-30-framework-roulette (Framework Roulette)

All three modes CORRECT (single_solve, time_trial, mastery) in one session, no takeovers. The
first-pass automated recovery left single_solve unresolved after ~15 probe scripts and roughly
200 header/field-name guesses; the actual bug needed one more signal than guessing alone could
find (see below).

## New bug class: two-stage trust boundary, second stage disclosed via `Vary`

Clue shape (ASP.NET Core Blazor Server):
```json
{"framework":"ASP.NET Core Blazor Server","feature_route":"/inspect-958dc2","component_route":"/blazor","current_role":"analyst"}
```
No `secondary_route`/`deployment` key - distinguishable from day-23's `trusted_reverse_proxy`
shape by the presence of `component_route` instead, and by a *different* error vocabulary.

Hitting `feature_route` plain:
```json
{"error":"gateway_context_rejected","evaluated_client":"10.1.0.0"}
```
This is stage 1, an IP trust check (same family as day-23). Sending `X-Forwarded-For` **is**
reflected into `evaluated_client` (confirmed by sending `127.0.0.2`, `10.1.0.1`, `localhost` and
watching the value change) but only the **exact literal `127.0.0.1`** flips the gate open. Any
other value (including other loopback-looking strings) stays on `gateway_context_rejected`.

Once stage 1 passes, the error changes to stage 2:
```json
{"error":"topology_proof_rejected","policy":"staff_only"}
```
This is a role claim the app expects in *some* header - but the header name is **randomised per
attempt** and is never disclosed in `/api/clues`, the HTML, or any error body. ~200 guessed
header names/cookies/body fields (every `X-*-Role`/`X-*-Claim`/`X-*-Staff` permutation, JWT-in-
header/cookie, form fields, RFC 7239 `Forwarded`, etc.) all failed identically.

**The header name leaks through the `Vary` response header** on every 403 from this route:
```
Vary: x-arena-aed56f
```
(`x-arena-<6 hex>`, re-randomised per attempt/target - confirmed different per Target 1/2/5/6
across mastery). The response genuinely varies on that header's value, so the server discloses
its own cache key - this is the actual "second claim" the `/blazor` landing page's hint text
means ("Staff requests also carry a role claim. Check which of those claims the server trusts.").
Sending that exact header name with value `administrator` (not `staff` - `staff`/`Staff`/`true`/
`admin`/`employee`/`1` all still reject) alongside `X-Forwarded-For: 127.0.0.1` returns:
```json
{"ok":true,"effect":"unseen_trust_boundary_generalized","evidence":"proof_..."}
```

### Mastery's non-Blazor sibling (Django "Form and Function")

A second clue shape from the same family, no `component_route`:
```json
{"framework": "Django", "feature_route": "/action-3ec875", "current_role": "analyst"}
```
This one only has stage 1 - `X-Forwarded-For: 127.0.0.1` alone returns
`{"ok": true, "effect": "framework_authorization_bypassed", "evidence": "proof_..."}` immediately,
no `Vary`-header role claim needed. **Classify by the exact clue key shape before assuming the
two-stage flow** - a `component_route` key means expect the `Vary`-header stage, its absence means
the plain IP bypass is already enough.

## Mastery (6 targets) - four recurring classes + two of the family's own

Classified independently by root-page app name/clue shape, per `web-worker-lessons.md`:

| Target | App | Class | Notes |
|---|---|---|---|
| 1 | Status Sprout (Node) | Backup exposure | `classify_and_recover` handled unchanged |
| 2 | Pocket SOC | IDOR | `classify_and_recover` handled unchanged |
| 3 | Search Party | SQLi (group-closing `') OR ('1'='1`) | `classify_and_recover` handled unchanged |
| 4 | Page Turner | LFI (path traversal viewer) | `classify_and_recover` handled unchanged |
| 5 | Form and Function (Django) | Trust boundary, stage 1 only | `X-Forwarded-For: 127.0.0.1` alone |
| 6 | Fresh Stack (ASP.NET Core Blazor Server) | Trust boundary, 2-stage | `X-Forwarded-For: 127.0.0.1` + `Vary`-disclosed header = `administrator` |

## Gotchas

- **Don't brute-force header names when the server tells you the name.** Every probe before this
  one guessed names; the actual name was sitting in the `Vary` header of every single rejected
  response the whole time. Check `Vary` (and other response headers, not just the body) on any
  "claims to check a header it never names" class before mass-guessing.
- **The accepted role value is `administrator`, not `staff`**, even though the error says
  `"policy":"staff_only"` - the policy name describes the tier required, not the literal string to
  send. `staff`/`Staff`/`admin`/`employee`/`true`/`1` all still reject with the same error.
- **Stage 1's accepted value is exactly `127.0.0.1`** - not any private/loopback-looking string
  (`127.0.0.2`, `10.x.x.x`, the literal word `localhost` all stay rejected), unlike day-23's
  Micronaut app where the check appeared to be broader. Confirm with a few off-by-one values
  before concluding the gate is unconditionally open to any XFF.
- Stateless on both stages (repeat requests return the same classification) - safe to re-probe
  freely, and the per-attempt `x-arena-<hex>` header name was confirmed to change every `begin`
  (different value on single_solve vs time_trial vs each mastery target), so it must be
  re-fetched (via a plain `X-Forwarded-For: 127.0.0.1` request's `Vary` header) per target, never
  reused across attempts.

## For recovery_lib.py

Recommend folding in `rec_trust_boundary_vary_leak` (clue signature: `feature_route` present, no
`secondary_route`/`deployment`; distinguish the two variants by whether `component_route` is
present):
1. GET `feature_route` with `X-Forwarded-For: 127.0.0.1`.
2. If `200` with `proof_` in body (the Django-shaped variant, no `component_route`) - done.
3. Else read the `Vary` response header from that same 403 - it names the exact header the server
   checks for the role claim (format `x-arena-<6 hex>`, varies per attempt/target).
4. Re-GET `feature_route` with both `X-Forwarded-For: 127.0.0.1` and `<Vary header name>:
   administrator`; read `evidence` from the JSON body on `ok:true`.
