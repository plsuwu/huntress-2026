---
day: 23
slug: "forwarded-misgivings"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [header-spoofing, auth-bypass, backup-exposure, idor, sqli, lfi, path-traversal, reverse-proxy-trust, x-forwarded-for, loopback-bypass, php-include, wrapper-prefix-block, recursive-include-crash, traversal-depth-sweep, group-closing, target-classification]
---

# Day 27 - web-exploitation-day-23-forwarded-misgivings ("Forwarded Misgivings")

All three modes CORRECT (single_solve, time_trial, mastery) in one session, no takeovers.

## New bug class: trusted-reverse-proxy header spoofing (Micronaut)

`/api/clues` on this family's own targets discloses:
```json
{"framework":"Micronaut","feature_route":"/action-xxxxxx","current_role":"analyst","deployment":"trusted_reverse_proxy"}
```
No `secondary_route`/`inventory_route`/`protected_reference` key - distinguishable from every
earlier clue-driven class by key shape alone (just `feature_route` + `current_role` +
`deployment`).

Hitting `feature_route` with no extra headers:
```json
{"error":"loopback_only","evaluated_client":"10.1.0.0"}
```
The app is deciding "am I being called from loopback" using whatever IP it thinks the client is -
and it trusts a client-supplied `X-Forwarded-For` header for that decision with **no actual
upstream-proxy allowlist**. Sending `X-Forwarded-For: 127.0.0.1` on the exact same request flips
`evaluated_client` to `127.0.0.1` and returns:
```json
{"ok":true,"effect":"proxy_trust_admin_bypass","evaluated_client":"127.0.0.1","evidence":"proof_..."}
```
Tried and ruled out: `X-Real-IP`, `X-Forwarded-Host`, `X-Forwarded-Proto` (alone or combined with
XFF) - none affect `evaluated_client`; XFF alone is both necessary and sufficient. Stateless check
(repeat requests return the same proof) - safe to re-probe freely.

This is a clean, minimal single-insight trigger in the same shape as every other `rec_*` in
`recovery_lib.py` - worth folding in as `rec_proxy_trust_bypass` (clue signature:
`deployment == "trusted_reverse_proxy"`, no `secondary_route`).

## Mastery (5 targets) - cumulative as predicted

Classified independently by root-page content, per `web-worker-lessons.md`'s "classify every
target independently" lesson - did not assume the family's own bug applied to all 5:

| Target | App | Class | Evidence |
|---|---|---|---|
| 1 | Status Sprout (Node) | Backup exposure | HTML comment "review crawler policy before launch" -> `/robots.txt` `Disallow: /releases-5a1cf/` -> open listing names `release-settings-*.json.bak` -> `support_note` field holds the proof |
| 2 | Incident Picnic | IDOR | Root links `/api/activity`; feed includes own cases (`visibility:"workspace"`) plus one foreign case (`visibility:"reference_only"`, `owner:"executive"`); same session cookie against `/api/tickets/<foreign_id>` returns the proof in `note` |
| 3 | Signal Shelf | SQLi (group-close) | `<form action="/lookup">` with input `q`; day-19's exact `') OR ('1'='1` payload reconfirmed verbatim (plain `' OR '1'='1` and `' OR 1=1--` both failed) |
| 4 | Stay in Your Directory (PHP/Apache) | LFI | `/handbook.php?doc=welcome.php` style viewer, no traversal sanitization; `/etc/passwd` readable at `../../../../etc/passwd` confirming the trick; proof needed the documented default objective path `run/huntress/objective.txt` at **3** levels of `../` (depth sweep 1-8, hit at depth 3) |
| 5 | Forward Desk (Micronaut) | Proxy trust bypass (family's own) | Same `/api/clues` -> `feature_route` -> `X-Forwarded-For: 127.0.0.1` as single_solve/time_trial |

This confirms the `web-worker-lessons.md` pattern again: backup exposure + IDOR +
group-close SQLi + LFI + the family's own novel bug, five targets, one of each class
(no repeats this time, unlike some earlier families that doubled up a class across multiple
mastery targets).

## Gotchas

- **`php://filter` does not work for source disclosure here.** `handbook.php`'s include is
  `include($pages_dir . $doc)` (string concatenation, not a validated join) - confirmed via a PHP
  warning leak: `include(/var/www/html/pages/<doc>): Failed to open stream`. Plain `../` traversal
  resolves fine through that concatenated path (confirmed against `/etc/passwd`), but prefixing
  `php://filter/convert.base64-encode/resource=...` just produces the literal invalid path
  `/var/www/html/pages/php://filter/...` - the wrapper scheme has to be the start of the string PHP
  hands to `include()`, and the fixed `pages_dir` prefix always wins. Don't waste requests on filter
  wrapper tricks against this app; a plain traversal path is the only lever.
- **Including a real `.php` file crashes the container.** Traversing to `../index.php` (one level
  up from `pages_dir`) returned Cloudflare `error code: 1101` - likely `index.php` re-dispatching
  into itself via the same `?doc=` param, recursing until the process dies. The target self-recovered
  within ~3s with no `wk.py begin` needed, but avoid this: only traverse to known-plain-text targets
  (the documented objective path, `/etc/passwd` for confirmation), never to `.php`/executable files
  under this app's webroot.
- Guessed filenames (`secret.php`, `flag.txt`, `objective.txt` at 1-2 traversal depths, etc.) all
  failed cleanly with a PHP "Failed to open stream" warning - distinguishable from a hit (which
  returns normal page content with the `proof_` evidence inline, no warning). The actual objective
  lived at `run/huntress/objective.txt`, 3 levels of `../` up from `pages_dir` - exactly
  `recovery_lib.py`'s documented default, confirming that default is reusable rather than
  app-specific.

## For recovery_lib.py

Recommend folding in `rec_proxy_trust_bypass` (clue signature: `deployment ==
"trusted_reverse_proxy"`, `feature_route` present, no `secondary_route`) - send
`X-Forwarded-For: 127.0.0.1` on `feature_route`, read `evidence` from the JSON body on `ok:true`.
`rec_lfi` as already documented (default objective path, 1-8 depth sweep, one absolute fallback)
worked unchanged here - no changes needed there, just confirmation it transfers to a fifth
independent app (now Apache/PHP `handbook.php?doc=`, after day-20's PHP viewer).
