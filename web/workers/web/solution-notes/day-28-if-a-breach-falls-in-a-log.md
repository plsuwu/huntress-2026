---
day: 25
slug: "if-a-breach-falls-in-a-log"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [missing-audit-log-event, broken-logging, security-event-suppression, backup-exposure, idor, sqli, lfi, path-traversal, group-closing, traversal-depth-sweep, ownership-bypass, robots-txt, directory-listing, target-classification, slim, php]
---

# Day 28 - web-exploitation-day-25-if-a-breach-falls-in-a-log ("If a Breach Falls in a Log")

All three modes CORRECT (single_solve, time_trial, mastery) in one session, no takeovers.

## New bug class: missing audit log event on a privileged action (Slim)

`/api/clues` on this family's own targets discloses:
```json
{"framework":"Slim","feature_route":"/process-xxxxxx","secondary_route":"/status-xxxxxx","current_role":"analyst"}
```
Bare two-key clue shape (feature_route + secondary_route), shared with SSRF and
double-spend - the distinguishing probe is a read-only GET on secondary_route:
this family's status route returns `{"ok":true,"events":[],"report":{"method":"POST","fields":["export_id","missing_event","observed_event"]}}`,
disclosing the report schema up front.

### Mechanism

1. `POST feature_route` with an empty JSON body performs a protected export:
   `{"ok":true,"export":"completed","export_id":"<hex>","operation":"protected_export"}`.
2. `GET secondary_route` shows the audit log: for that `export_id` only a benign
   `page_view` event was recorded - the decisive `protected_export` event was
   never logged ("if a breach falls in a log...").
3. `POST secondary_route` with the report body disclosed by `report.fields`
   (`{"export_id": <id>, "missing_event": "protected_export", "observed_event": "page_view"}`)
   returns the proof: `{"ok":true,"effect":"decisive_security_event_missing","evidence":"proof_<hex>"}`.

The field names are not hard-coded: `report.fields` names them (`export_id`,
`missing_event`, `observed_event`), the export response supplies the id and the
operation name, and the audit-log listing supplies the observed event.

## Mastery (5 targets)

Exactly the cumulative mix predicted by `docs/lane-patterns.md` after day-27:
backup exposure + IDOR + day-19's group-closing SQLi (`') OR ('1'='1`, verbatim)
+ day-20's LFI (bounded `../` depth sweep 1-8 + absolute fallback) + this
family's own missing-audit-log class. One target per class, no repeats, 5 total.

## Gotchas

- The feature route is POST-only (GET returns 405 `post_required`).
- Always GET secondary_route first: it is read-only and discloses both the bug
  class (via `report.fields`) and the observed event needed for the report.
  This probe also safely distinguishes this family from double-spend (whose
  feature_route is one-shot and must never be probed serially) and from SSRF
  (whose secondary_route 403s direct external requests).
- mastery target classification order: clues-driven classes first (with the
  secondary-route probe separating day-25 from SSRF/double-spend on the same
  bare shape), then PHP-viewer LFI, IDOR, form-based SQLi, backup exposure last.
