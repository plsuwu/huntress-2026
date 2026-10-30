---
day: 28
slug: "pass-the-identity"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [delegated-identity, identity-spoofing, header-spoofing, broken-access-control, confused-deputy, axum, backup-exposure, idor, sqli, group-closing, lfi, path-traversal, ownership-bypass, robots-txt, directory-listing, target-classification]
---

# Day 30 - web-exploitation-day-28-pass-the-identity ("Pass the Identity")

All three modes were graded CORRECT on their first submissions. Single Solve and
Time Trial each had one target; Mastery had five. All routes were played untimed,
and no reset or target recovery was needed.

## New bug class: client-forged delegated identity (Axum)

The app is "Caller Courier". `/api/clues` has a distinctive new key shape:

```json
{"current_role":"analyst","feature_route":"/execute-<hex>",
 "framework":"Axum","identity_directory_route":"/api/identities",
 "secondary_route":"/result-<hex>"}
```

`GET identity_directory_route` discloses the current caller and the identity that
owns the protected records:

```json
{"caller":"analyst","records_owner":"executive"}
```

A request to `feature_route` without identity material returns HTTP 403
`{"error":"delegated_identity_rejected"}`. The delegated boundary trusts a
client-supplied user header without authenticating it. Send the disclosed records
owner verbatim in that header:

```http
GET <feature_route>
X-Delegated-User: <records_owner>
```

For the observed target, the value was `executive`. The response was:

```json
{"effect":"delegated_identity_trusted","evidence":"proof_<hex>","ok":true}
```

Thus the protected effect requires only forging `X-Delegated-User`; no token,
signature, session change, SSRF, or direct request to `secondary_route` is needed.
The exact header name matters: similarly named caller, identity, principal, role,
and forwarding headers did not change the 403 baseline. Derive the owner from the
live identity directory rather than hard-coding `executive`.

### Dispatcher gotcha and recoverer candidate

The first pass misclassified this family as SSRF. Its bare
`feature_route`/`secondary_route` branch treats any secondary-route HTTP 403 as
SSRF, but this response says `delegated_identity_rejected`, not
`internal_service_only`. Every subsequent loopback-port probe returned that same
identity rejection.

A future `rec_delegated_identity` should key on `feature_route` plus
`identity_directory_route` and run **before** the generic feature/secondary
branch. It should GET the directory, read `records_owner`, GET the feature with
`X-Delegated-User` set to that value, and extract `evidence`. The canonical SSRF
classifier should also require the internal-service evidence rather than status
403 alone. This family's local hook implements the minimal recovery; no shared
library files were edited.

## single_solve

The forged delegated-user request recovered
`proof_1c6e523b339a4ac1ac693383f0f94a1f`, accepted as `proof-1` on the first
submission. The identity-directory baseline and successful response are preserved
under `single_solve/evidence/Target_1/`.

## time_trial

The same generic solver derived the fresh target's routes and records owner,
recovered `proof_8e544c1f7d9d4c28a8a440f4da15b8a4`, and received CORRECT on the
first untimed submission. No Single Solve proof or URL was reused.

## mastery

Five targets, one per class, matching the established cumulative pool:

1. **Target 1 / proof-1 - backup exposure:** crawler-policy hint -> robots.txt
   Disallow -> directory listing -> named backup;
   `proof_363b21bb480e41c8b1c0edc4ce0f8059`.
2. **Target 2 / proof-2 - IDOR:** activity feed disclosed a foreign object id;
   the object API lacked ownership enforcement;
   `proof_8ce3715f77c64e778bae1b02b4235314`.
3. **Target 3 / proof-3 - group-closing boolean SQLi:** the exact
   `') OR ('1'='1` payload exposed the hidden proof row;
   `proof_4ca8a28c3b9441078a8603337dce0553`.
4. **Target 4 / proof-4 - PHP LFI:** bounded plain traversal reached
   `/run/huntress/objective.txt`;
   `proof_f48c70165d5a40669012392918ee3af9`.
5. **Target 5 / proof-5 - this family's delegated-identity spoof:** the identity
   directory disclosed `records_owner`, and `X-Delegated-User` released
   `proof_c73adedcc3144f238e66dba0780681b7`.

Each question was joined to its target through `questions[].artifact`, with a
separate cookie jar and evidence directory per target. All five answers were
accepted together on the first submission. Evidence was captured before submit.

## Files

- `single_solve/`, `time_trial/`, `mastery/`: start data, raw HTTP evidence,
  answers, grading rounds, and per-mode NOTES.md.
- `solve.py`: the shared all-mode runner with only `rec_this_family()` changed;
  copied verbatim to `past-solutions/day-28-pass-the-identity-solve.py`.
