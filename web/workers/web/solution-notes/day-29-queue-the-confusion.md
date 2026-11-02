---
day: 29
slug: "queue-the-confusion"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [queue-owner-spoofing, missing-owner-binding, ownership-bypass, broken-access-control, pyramid, backup-exposure, robots-txt, directory-listing, idor, sqli, boolean-sqli, group-closing, lfi, path-traversal, django, auth-bypass, target-classification]
---

# Day 30 — web-exploitation-day-29-queue-the-confusion (Queue the Confusion)

All three modes were CORRECT on their first submissions. Time trial and mastery were regular untimed attempts; no timed, retry, practice, or reset path was used.

## single_solve

### New bug class: queue-owner binding bypass

The Pyramid app (Job Post) exposes this clue shape:

```json
{"framework":"Pyramid","feature_route":"/process-<random>","secondary_route":"/audit-<random>","owner_directory_route":"/api/owners","current_role":"analyst"}
```

The owner directory discloses `caller` and `restricted_owner`. A read-only GET of `feature_route` returns 405 `post_required` plus the exact `request_fields` (one field, `owner`). A GET of `secondary_route` without an id returns 400 `job_id_required` plus `query_parameter:"id"`.

The queue accepts the restricted owner directly from the analyst's POST body instead of binding the job to the caller:

1. `GET owner_directory_route`; select `restricted_owner` (different from `caller`).
2. `POST feature_route` with the disclosed owner field set to that restricted owner. The 201 response returns a job `id` and `queued_owner`.
3. `GET secondary_route?<disclosed id parameter>=<returned job id>`.
4. The audit response returns `effect:"queue_owner_binding_bypassed"` and `evidence:"proof_<hex>"`.

A baseline analyst-owned job instead produced 403 `job_not_authorized` at the audit route, confirming that the foreign queued owner is decisive. This is not a race and not a one-shot credit: repeated jobs are safe, and the minimal exploit needs only the disclosed `owner` field.

Single_solve submitted `proof_c368799376734548afb7d2c691723932` and was CORRECT.

## time_trial

The same all-mode solver recovered the fresh target's proof, `proof_23d03b52a43b4dd8a1b581e32ae07a52`, on the first untimed submission. All routes, parameters, owner values, and job ids were re-derived; no per-attempt data was reused.

## mastery

Mastery contained six independently classified targets, one per class:

1. **Backup exposure (Status Sprout):** crawler-policy hint -> robots disallow -> open directory -> named JSON backup; `proof_3b8fd8e918a045e6801cc3252fd3161d`.
2. **IDOR (Case Crate):** activity feed disclosed foreign case `17196`; the same-session ticket API returned it; `proof_fe316d9d03ca4393b49449545f5b02a5`.
3. **Group-closing SQLi (Search Party):** the exact `') OR ('1'='1` payload against the derived `/query?q=` surface exposed the hidden row; `proof_429e5253c0984bc08996eea3dead307f`.
4. **PHP LFI (Runbook Nook):** derived `/reader.php?doc=` and recovered the objective at traversal depth 3; `proof_8c0234d01d29421cb8bc46835ec1a8c9`.
5. **Django framework authorization bypass (Form and Function):** the exact clue shape was only `framework`, `feature_route`, `current_role`. Direct read-only GET of `/submit-4c797d` returned `effect:"framework_authorization_bypassed"` and `proof_4ceee731985745b7a8a338a87859ddc3`.
6. **This family's queue-owner binding bypass (Object Queue):** `proof_a937999e55cd4a0d9ce152cfb1b7fe0c`.

The shared runner used separate cookie jars/evidence directories and recovered all six before one batch submit. Its verbatim copy is `past-solutions/day-29-queue-the-confusion-solve.py`.

## Coordinator recoverer candidate

Add a `rec_queue_owner_binding(t, clues)` keyed on `feature_route` + `secondary_route` + `owner_directory_route`. It should derive the restricted owner, feature request field, audit query parameter, and returned job id as above, then extract the audit response's proof. Dispatch it **before** the ambiguous bare `feature_route` + `secondary_route` branch: the existing branch probes the audit route, sees 400 `job_id_required`, and otherwise fails closed as unclassified.

The mastery-only bare Django shape was handled in the local hook with a read-only feature-route GET. If the other day's worker has not already supplied that recoverer, its exact observed signature and effect are recorded above for coordinator review; no shared library was edited by this worker.
