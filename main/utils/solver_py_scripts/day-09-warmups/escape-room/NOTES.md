# Day 9 warmups - Escape Room (`warmups-day-09`)

Same slug-in-encoded-text template as day-05..08; the layer is **backslash string escapes**
(`"Ni..."` - JSON/JS `\uXXXX` for every byte, wrapped in double quotes) -> "Nice work decoding this warmup! ... submit ...: <slug>".

## single_solve (DONE, CORRECT - flag d5f62618...)
- `artifacts/single_solve-blob.txt`, answer id `message`, slug `happy-piano-whale-26`.
- `solve.py` = day-08 superset + `unescape_bs` (`\uXXXX`, `\u{X}`, `\UXXXXXXXX`, `\xNN`, octal `\NNN`, `\n\t\\` etc,
  surrogate pairs recombined, surrounding quotes stripped, repeated while escapes remain), in single path and `decode_row`.
- Route has `run_for_time_supported: true` (time_trial deadline 7s, mastery 9s, 43 sets, pool 128).

## time_trial (DONE, CORRECT - flag ab75ed0b...) / mastery (DONE, CORRECT - flag b22e546b...)
- `run_timed.py <time_trial|mastery> --challenge warmups-day-09 [--retry] [--untimed]` (`set -a; . ./.env; set +a`, `python3 -I`).
  time_trial 5.0s total (download 3.1s), mastery 5.4s (43 downloads 3.4s). 127 ranked attempts left after each; no retries burned.
- Offline regression done before the timed mastery: solve.py still decodes the day-08 mastery fixture (38/38) and synthetic escape rows.
- Bug caught offline: do NOT recurse `decode_row` after unescaping a row - the unescaped text is a plain fragment, re-decoding
  it as base64/rot13 corrupts it.
- Variants dumped under `artifacts/time_trial-*/` and `artifacts/mastery-*/`.
- Mastery blob format (43 per-question blobs, `NN<TAB>fragment` rows, 9 rows): base64, rot13, hex, 8-bit binary, decimal,
  octal, percent, HTML entities, and (day-9 addition) backslash escapes - all handled by `decode_row`.
