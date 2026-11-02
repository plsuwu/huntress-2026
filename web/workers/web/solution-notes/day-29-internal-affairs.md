---
day: 27
slug: "internal-affairs"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [ssrf, idor, ownership-bypass, loopback-access, chained-exploitation, broken-access-control, backup-exposure, robots-txt, directory-listing, sqli, group-closing, lfi, path-traversal, ktor, target-classification]
---

# Day 29 — web-exploitation-day-27-internal-affairs ("Internal Affairs")

All three modes were CORRECT on their first submissions in one session. No timed, retry, practice, or reset path was used.

## single_solve

### New bug class: SSRF chained to internal IDOR (Kotlin Ktor)

`/api/clues` has a new, recognizable shape:

```json
{
  "framework": "Ktor",
  "feature_route": "/submit-<random>",
  "case_directory_route": "/api/cases",
  "topology_route": "/api/topology",
  "current_role": "analyst",
  "service_pattern": "server-side case fetch"
}
```

The exploit crosses two authorization boundaries:

1. GET `topology_route`. Its `case_service` value is an absolute loopback URL such as `http://127.0.0.1:8080/status-<random>`. Requesting that status path directly from outside returns 403 `internal_service_only`.
2. GET `case_directory_route`. It returns numeric case ids and owners; select an id whose owner differs from `current_role`.
3. GET `feature_route` without parameters. The 400 `target_required` response discloses `query_parameter: target`.
4. Add `id=<foreign case id>` to the disclosed `case_service` URL, URL-encode that complete URL as the outer `target` parameter, and GET the feature route. The public server fetches the loopback service, which returns the foreign record without checking ownership. Its response contains `effect: ssrf_then_object_authorization` and `evidence: proof_<hex>`.

The proof comes from the *foreign* id, not from the internal status URL alone. Fetching that URL without `id` returns 404 `not_found`; `?id=<own analyst id>` returns the caller's ordinary case and confirms the lookup contract.

### Gotchas

- The internal record parameter is exactly `id`; `case_id` was tried and produced the same 404 as an omitted id.
- Encode the nested internal URL, including its own `?id=...`, as one outer fetch-parameter value.
- Derive the loopback URL and port from `topology_route`; do not sweep ports for this family.
- Strip the assigned target URL's trailing slash before concatenating routes.
- A direct request to the internal-only status path is a safe classifier: it returns 403, whereas the SSRF path reaches the same application and its case lookup.

The generic solver is `past-solutions/day-29-internal-affairs-solve.py`.

## time_trial

The same generic solver recovered the fresh target's proof on the first untimed submission. It derives every route, parameter, service URL, and foreign id from live responses; no single_solve answer or target URL was reused.

## mastery

Mastery had five targets, one per class, matching the cumulative pool:

1. Backup exposure: root crawler hint -> `robots.txt` -> disclosed directory listing -> named backup file.
2. IDOR: activity feed disclosed a foreign object id; the object API returned it under the same session.
3. Group-closing SQLi: the exact `') OR ('1'='1` payload exposed the hidden proof row.
4. PHP LFI: derive the viewer endpoint and parameter from its link, then use a bounded `../` depth sweep for `/run/huntress/objective.txt`.
5. This family's own SSRF -> internal foreign-case IDOR chain described above.

The authored classifier joined each question's `artifact` to `targets[].id`, used a separate cookie jar and evidence directory per target, and recovered all five proofs before one batch submission. Its verbatim copy is `past-solutions/day-29-internal-affairs-mastery.py`.

### Recoverer candidate

A future `rec_internal_affairs` can key on the simultaneous presence of `feature_route`, `topology_route`, and `case_directory_route`. It should derive `case_service`, the outer parameter from `feature_route`'s 400 response, and the foreign id from the case directory, then append `id=<foreign id>` to the internal URL before passing it through the public fetch. This is one tight, stateless trigger; no field-name matrix or one-shot state is involved.
