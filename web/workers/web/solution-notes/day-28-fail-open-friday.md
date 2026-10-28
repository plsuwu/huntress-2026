---
day: 24
slug: "fail-open-friday"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [fail-open, error-handling, auth-bypass, policy-decoder, base64url, exceptional-condition, backup-exposure, idor, sqli, lfi, path-traversal, group-closing, robots-txt, directory-listing, ownership-bypass, traversal-depth-sweep, asp-net-core, url-normalization, target-classification]
---

# Day 28 - web-exploitation-day-24-fail-open-friday ("Fail Open Friday")

All three modes CORRECT in one session (one coordinator `op=reset` on mastery after
a self-inflicted target crash - see the double-slash lesson below).

## New bug class: fail-open remote policy decoder (ASP.NET Core Razor Pages)

`/api/clues` discloses a NEW key shape:
```json
{"framework":"ASP.NET Core Razor Pages","feature_route":"/inspect-xxxxxx",
 "policy_format_route":"/api/policy-format","current_role":"analyst",
 "expected_behavior":"deny access when the policy service is unavailable"}
```
`policy_format_route` (new - no other class uses it) discloses the whole token
grammar: query-parameter name (`policy`), a per-attempt string prefix
(`remote:<hex>:`), the encoding (base64url UTF-8 JSON), and a documented *valid
public* decision plus its encoding (`{"Scope":"public","Allow":true}`).

The access decision on `feature_route` decodes the policy token:
- Valid public decision -> `{"ok":true,"policy":"standard"}` (no proof).
- Token with a well-formed prefix but an **undecodable payload**
  (`<prefix>not-valid-base64!!!`, or base64url of non-JSON bytes) ->
  `{"ok":true,"effect":"exceptional_condition_fail_open","evidence":"proof_..."}`.

The decoder's exception path FAILS OPEN and grants the protected effect - the
exact opposite of the disclosed `expected_behavior` ("deny access when the policy
service is unavailable"). Stateless check; safe to re-probe. Both the parameter
name and prefix are per-attempt: derive from `policy_format_route`, hard-code
nothing. The family's own app is titled "Recovery Gate" in the mastery variant.

Distinguishing clue shape: `policy_format_route` present (with `feature_route`)
and no `secondary_route`/`inventory_route`/`protected_reference`/`deployment` -
unique among all feature_route-driven classes so far.

## Mastery (5 targets, same cumulative shape as day 27)

Backup exposure (Tiny Uptime: robots.txt Disallow -> dir listing -> `*.json.bak`)
+ IDOR (Incident Picnic: /api/timeline discloses foreign case, /api/cases/<id>
with own session) + group-closing SQLi (`') OR ('1'='1` on /query?q=, first try
again) + LFI (/reader.php?doc=, `../`x3 to /run/huntress/objective.txt) + the
family's own fail-open decoder. One target per class, no repeats.

## The big operational lesson: normalize `targets[].url` before concatenating paths

`targets[].url` has a **trailing slash**. `f"{url}/cases/4522"` -> `//cases/4522`,
and these apps route on the raw path: double-slash requests 404 on the IDOR app,
fall through to the catch-all fallback on the backup app (making the real
/robots.txt look "missing"), and reach a broken handler on the PHP LFI app
(reader.php answering a constant `basename(null)` "not found" that looks exactly
like a parameter-parsing/variables_order bug). Three healthy apps were
"diagnosed" as degraded for ~40 minutes; T1's container was additionally
crash-looped by extensionless-path probes during the phantom hunt and needed a
coordinator `op=reset` (the day-27 documented path, user-directed). On the fresh
targets with `url.rstrip('/')` every class resolved on the first textbook
attempt. **Strip the trailing slash; if every route but `/` 404s, check for a
double slash before anything else.** `recovery_lib.Target` already strips it -
any hand-rolled client must do the same.

Secondary notes: `Accept: text/html` 404s being a different size than `Accept:
*/*` 404s is a Cloudflare-insights-script rendering artifact, not a routing
signal; an app's root `/robots.txt` may be Cloudflare's boilerplate
content-signals file (no Disallow) - not app evidence; proofs are
per-target-instance, so after a reset re-derive every answer before submitting.

## Files

- `single_solve/`, `time_trial/`, `mastery/` - per-mode evidence + NOTES.md
- `solve.py` - generic fail-open solver (all routes/params derived live), also at
  `past-solutions/day-28-fail-open-friday-solve.py`
