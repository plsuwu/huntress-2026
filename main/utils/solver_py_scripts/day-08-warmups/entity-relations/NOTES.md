# Day 8 warmups - Entity Relations (`warmups-day-08`)

Same slug-in-encoded-text template as day-05/06/07; the layer is **HTML entities**
(`&#78;&#105;...` decimal numeric refs for every byte -> "Nice work decoding this warmup! ... submit ...: <slug>").

## single_solve (DONE, CORRECT - flag 1a1b4e54...)
- `artifacts/single_solve-blob.txt`; answer id `message`; slug `word-word-word-NN` (jovial-rocket-marble-82).
- `solve.py` = day-07 superset + `html.unescape` loop (decimal/hex/named entities, repeated) in single and `decode_row`.
- Route has `run_for_time_supported: true` for time_trial/mastery: plain `start` = regular untimed attempt;
  `run_for_time:true` = ranked timed (deadline 8s; pool 128; mastery 38 sets).
- `run_timed.py <time_trial|mastery> [--retry] [--untimed]` (day-07 runner + run_for_time). `set -a; . ./.env; set +a`, `python3 -I`.

## time_trial (DONE, CORRECT - flag 90520e0b...) / mastery (DONE, CORRECT - flag e0382232...)
- Both run with `run_for_time:true` (hall_of_fame_eligible). time_trial total 7.2s (deadline 8s - tight; download ~3.2s),
  mastery 6.6s (38 downloads ~3.6s). 128 ranked pool; 127 left after each. First timed try worked, no retries burned.
- Variants dumped under `artifacts/time_trial-*/` and `artifacts/mastery-*/`; the day-07 row decoders plus the entity
  branch solved mastery unchanged.
- Time trial headroom is thin (~0.8s); if a future run risks expiry, drop the post-submit artifact dump (already after submit)
  or reduce work before submit.
- Mastery blob format (38 per-question blobs, `NN<TAB>fragment` rows, 8 rows): base64, rot13, hex, 8-bit binary, decimal, octal,
  percent, HTML entities (decimal `&#NN;`). Row 7 (percent) and 8 (entities) are the day-07/08 additions.
