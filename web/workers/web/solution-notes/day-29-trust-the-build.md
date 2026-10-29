---
day: 26
slug: "trust-the-build"
lane: web-exploitation
modes: [single_solve, time_trial, mastery]
has_solve_py: [single_solve, time_trial, mastery]
tags: [unverified-update, dependency-tampering, integrity-check-bypass, missing-artifact-verification, hook-tampering, backup-exposure, idor, sqli, lfi, path-traversal, group-closing, robots-txt, directory-listing, ownership-bypass, traversal-depth-sweep, target-classification, fiber]
---

# Day 29 - web-exploitation-day-26-trust-the-build ("Trust the Build")

All three modes were graded CORRECT on their first submissions. Single Solve and
Time Trial each had one target; Mastery had five. No reset or target recovery was
needed.

## New bug class: unverified dependency artifact / update-hook tampering (Fiber)

`/api/clues` discloses a distinctive key shape:

```json
{"current_role":"analyst","feature_route":"/inspect-<hex>",
 "framework":"Fiber","update_feed":"/api/updates",
 "update_policy":"/api/update-policy"}
```

The update feed returns complete package update objects with `package`, `version`,
an `artifact` object (`build` and `hook`), `artifact_sha256`, and `signature`. The
policy route names the routine and restricted hooks and explicitly states the
integrity contract: activation must verify artifact bytes against the signed
manifest's SHA-256.

A baseline POST of the intact update object to `feature_route` produces
`routine_update_activated`. To recover the proof, copy that update and change only
`artifact.hook` to the policy's restricted hook (observed: `privileged_export`).
Keep the original `artifact_sha256` and `signature` unchanged. The endpoint returns:

```json
{"effect":"unverified_update_activated","evidence":"proof_<hex>",
 "ok":true,"package":"<live package name>"}
```

Thus the updater does not enforce the disclosed artifact-integrity contract when
activating the supplied dependency. This is not signature forgery and does not
require a higher version or a different package: a one-field artifact modification
is sufficient. Derive the feature, feed, policy, package update, and restricted
hook live; do not hard-code the per-attempt route or proof.

Distinguishing clue shape: `update_feed` and `update_policy` together with
`feature_route`. The check was repeatable across the family's Single Solve, Time
Trial, and Mastery targets.

## single_solve

The intact update established the routine baseline. The one-field hook tamper then
returned the Target 1 proof, which was accepted as `proof-1` on the first submit.
Raw baseline and exploit responses are preserved in `single_solve/`.

## time_trial

The generic `solve.py` derived the fresh target's routes, update, and restricted
hook, recovered its proof, and received CORRECT on the first untimed submission.

## mastery

Five targets, one per class, matching Day 28's cumulative shape:

1. Backup exposure: robots.txt Disallow -> directory listing -> named backup.
2. IDOR: activity feed discloses a foreign object id; object API lacks ownership
   enforcement.
3. Group-closing boolean SQLi: the exact `') OR ('1'='1` payload worked first on
   the form-derived query route.
4. PHP LFI: plain bounded traversal to `/run/huntress/objective.txt`.
5. Unverified dependency update: the family's own artifact-hook tamper above.

`mastery.py` classifies each target independently, writes separate evidence per
Target N, and maps question ids through `questions[].artifact`. All five proofs
were accepted together on the first submission.

## Files

- `single_solve/`, `time_trial/`, `mastery/`: start data, raw HTTP evidence,
  answers, grading rounds, and per-mode NOTES.md.
- `solve.py`: generic single-target update-tampering solver; copied verbatim to
  `past-solutions/day-29-trust-the-build-solve.py`.
- `mastery.py`: independent five-target classifier/recoverer; copied verbatim to
  `past-solutions/day-29-trust-the-build-mastery.py`.
