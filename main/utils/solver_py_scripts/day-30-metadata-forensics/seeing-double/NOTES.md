# Day 30 metadata - Seeing Double (`metadata-forensics-day-30-seeing-double`)

Prompt: "The corpus is too large for manual browsing. Build a relationship map from exact identity, semantic identity,
containment, and claimed provenance - then expose the fabricated cluster." As usual only one new question kind came
out of it: single-solve asks it once, TT is the cumulative bundle (35 q), and mastery has **14 sets** (490 q).

## New kind: `d30-cluster-001-relation-identical-sha256-groups`
- Evidence: `cluster-001.zip` = `delivery-manifest.json` (`huntress.artifact_delivery_manifest.v1`: per artifact
  `id`, `filename`, `path` (`artifacts/<file>`), `content_type`, `family`, `producer_id` (`text_template`,
  `duplicate_ref`, `structured_image`, `structured_zip`), `size_bytes`, `sha256`) + `artifacts/*` (txt, ps1, json,
  png, zip). Single: 5 families x 6 byte-identical copies; TT/mastery: random ids (`amber-03`, `orchard-57`, ...)
  with a few groups of 2-6 among singletons.
- Graded: groups of **manifest `id`s** (not paths, not filenames - both were rejected) whose bytes have the same
  SHA-256; **singletons dropped**, ids sorted inside a group, groups ordered by their first id (not by hash).
  answer_format `json` ("compact valid JSON array with exact values and order"), `case_insensitive`.
- New `ctf-run/src/formats/cluster.rs` (manifest + files, hashes from the real bytes; files missing from the
  manifest get id = file stem). `lanes/metadata.rs::relation_answer` dispatches on `-relation-<field>` after x509.
  Only `identical-sha256-groups` is graded; `semantic-identity-groups` (text: BOM/CRLF/trailing whitespace; JSON:
  key order/whitespace; PNG: IHDR + inflated IDAT; zip: sorted member names + semantic member hashes),
  `claim-mismatch-ids` (manifest sha256/size vs bytes), `containment-pairs` (artifact zip member byte-identical
  to another artifact) are guesses. An unknown field fails with a `cluster_facts` dump (id, path, family,
  producer, claimed vs real sha256/size, semantic key hash per artifact).
- The single-solve was graded CORRECT by a manual curl submit (format search), so its ctf-run run says
  `already_solved` and `fixtures_from_runs.py` skips it; `fixtures/metadata/d30-single_solve.json` was written by hand.
- Predicted follow-ups (the prompt lists them): `semantic-identity-groups`, containment (zip member == another
  artifact), claimed provenance (`producer_id: duplicate_ref` whose bytes differ from the referenced original, or a
  manifest sha256 that the bytes contradict) and "the fabricated cluster" (a group id / list of ids).

## Runs (all untimed, all CORRECT on the first submit after adding the kind)
- `ctf-run/runs-d30/.../single_solve-01cbb5c2-1793365376`, `time_trial-1dff7b87-1793365390`,
  `mastery-ee969a2b-1793365403` -> `fixtures/metadata/d30-{single_solve,time_trial,mastery}.json` (replay 82/82 clean).
