---
day: 31
slug: "full-stack-incident"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [ssrf, unsigned-event, worker-dispatch, incident-reconstruction, detector-repair, request-log-analysis, broken-access-control, backup-exposure, idor, sqli, group-closing, lfi, path-traversal, auth-bypass, trailing-slash-bypass, django, reflected-xss, stored-xss, target-classification]
---

# Day 31 — web-exploitation-day-31-full-stack-incident ("Full Stack Incident")

All three modes were CORRECT on their first worker submissions in one session. No timed, retry, practice, or reset path was used. The generic all-mode solver is `past-solutions/day-31-full-stack-incident-solve.py`.

## single_solve

### New bug class: unsigned fetch-event to worker dispatch, with detector/repair response

The Actix Web target exposes this distinctive `/api/clues` shape:

```json
{
  "framework": "Actix Web",
  "current_role": "analyst",
  "feature_route": "/dispatch-<random>",
  "secondary_route": "/audit-<random>",
  "final_route": "/worker-<random>",
  "request_log_route": "/api/requests",
  "evidence_route": "/api/evidence",
  "response_route": "/api/response",
  "fetch_query_field": "target",
  "worker_query_field": "command",
  "event_json_field": "token"
}
```

The automated first pass saw the 403 from `secondary_route` and selected ordinary SSRF. Every loopback-port attempt returned `signed_fetch_required`; no port sweep is involved in the real solution. Reproduce the logged incident instead:

1. GET `request_log_route`. Select the record with `kind: "fetch"`, `signed: false`, `outcome: "queued"`, and a `target`. That target value is the event token; all routes and the token are per-attempt data.
2. GET `feature_route` with `fetch_query_field=<token>`. The response is 202 `fetch_event_created`. Other targets require a signed fetch.
3. POST `secondary_route` with JSON `{<event_json_field>: <token>}`. A GET is not equivalent. The 202 `worker_dispatched` response discloses `available_commands`, `resources`, and `worker_route`.
4. Materialize the resource command template (observed: `cat <resource>` plus resource `objective`, yielding `cat objective`). GET the disclosed worker route with `worker_query_field=<materialized command>`. It returns 202 `unsigned_worker_dispatched`.
5. GET `evidence_route`. Its `events` now include the unsigned worker dispatch and a signed benign status check. It also discloses `regression_goal` and `response_schema`.
6. POST `response_route` with a detector for the unsigned attack event and a repair that blocks internal fetches/unsigned events while allowlisting the signed status command:

```json
{
  "detector": {"kind": "worker.dispatch", "signed": false},
  "repair": {
    "block_internal_fetch": true,
    "require_signed_event": true,
    "worker_allowlist": ["status"]
  }
}
```

The exact `kind` and allowlisted command should be read from the evidence events, not hard-coded. A correct response returns `effect: incident_detected_and_remediated`, `evidence: proof_<hex>`, and all five regressions true: attack detected, benign status excluded, internal fetch blocked, status still allowed, and unsigned event rejected.

Gotchas established live:

- The request log is static; `/api/evidence` is the dynamic reproduction/evidence surface.
- The audit/secondary route requires the token in a POST JSON body. Query parameters and GET return `event_not_verified`.
- After audit dispatch, the worker command is a bare GET query. Adding the token to that worker request produced `worker_preconditions_missing`.
- The worker's `unsupported_command` response usefully discloses `resources` and `usage`, but the audit response already supplies the complete command template.
- Strip the target URL's trailing slash before joining routes, as usual.

## time_trial

The same generic hook recovered the fresh target's proof on the first untimed submission. It derived the randomized routes, queued token, command/resource, attack event kind, and signed status command from current responses. No single_solve proof or URL was reused.

## mastery

Mastery assigned **eight** targets, demonstrating that the cumulative pool is still growing and must always be read from `questions`/`targets` rather than assumed:

1. Backup exposure via robots.txt and the disclosed backup file.
2. IDOR via the activity feed's foreign object id.
3. Group-closing boolean SQLi using the canonical `') OR ('1'='1` payload.
4. PHP LFI via the disclosed viewer and bounded traversal depth.
5. Day-05 Django authorization bypass: GET the disclosed feature route without a trailing slash.
6. Reflected XSS plus isolated browser callback.
7. Stored XSS plus moderator bot/receipt route.
8. This family's unsigned fetch-event-to-worker chain above.

Canonical `recovery_lib.py` recovered 1–4 and 6–7. Target 5 was the only initially unclassified target because the historical Django `rec_admin` recoverer is not folded into the canonical module; a narrow local fallback requesting `"/" + feature_route.strip("/")` recovered it. The local family hook recovered target 8. All eight answers were then submitted in one batch and graded CORRECT.

### Recoverer candidates for the coordinator

- Add a `rec_full_stack_incident`-style recoverer before the generic `feature_route + secondary_route` dispatcher branch, keyed on the simultaneous presence of `final_route`, `request_log_route`, `evidence_route`, `response_route`, `fetch_query_field`, `worker_query_field`, and `event_json_field`. It should implement the exact evidence-led chain above. Do not let the secondary route's bare 403 select `rec_ssrf` for this shape.
- Consider folding in the older day-05 `rec_admin` as well: the exact Django clue shape reappeared in this mastery pool and is not currently handled by `classify_and_recover`.
