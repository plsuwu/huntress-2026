# Day 7 warmups - Percent Chance (`warmups-day-07`)

Same slug-in-encoded-text template as day-05/06; this time the layer is **percent/URL encoding**
(`%4e%69%63%65...` -> "Nice work decoding this warmup! ... submit ...: <slug>").

## single_solve (DONE, CORRECT - flag 8d75b26b...)
- `artifacts/single_solve-blob.txt`: every byte as `%XX`. Answer id `message`, slug `word-word-word-NN`.
- `solve.py` = day-06 solver superset: loops `unquote` while `%XX` remains (handles double-encoding),
  then falls back to numeric-base decoding; `decode_row` gained a percent branch for mastery rows.

## time_trial (DONE, CORRECT - flag 266a258d...) / mastery (DONE, CORRECT - flag 9a92a828...)
- Deadlines: time_trial 7s, mastery 9s (33 sets, `downloads` list of 33 per-question blobs). 256 ranked pool; 255 left.
- `run_timed.py <time_trial|mastery> [--retry]` (copy of day-06 runner, default challenge `warmups-day-07`;
  `set -a; . ./.env; set +a`, then `python3 -I`). Observed: time_trial 5.5s, mastery 5.5s (downloads ~3.3s).
- Mastery blobs are the *same* 6-row format as day-06 (base64, rot13, hex, 8-bit binary, decimal, octal;
  `NN<TAB>fragment`) - no percent rows; the day-06 row decoders solved it on the first timed try.
  Variants dumped under `artifacts/mastery-*/`. (Only time_trial/single use percent encoding.)
- Future days: expect another single-layer encoding (base32? html entities? punycode?). Add to `decode_row`/`solve`
  and dry-run on saved fixtures before the timed start.
