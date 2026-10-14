# Day 6 warmups - Eight Is Enough (`warmups-day-06`)

Sibling of day-05 `by-the-numbers`: same slug-in-encoded-text format, new base ("eight fingers" = **octal**).

## single_solve (DONE, CORRECT - flag f96ddf74...)
- `artifacts/single_solve-blob.txt`: whitespace-separated 3-digit octal ASCII codes (`116 151 143 ...`) ->
  "Nice work decoding this warmup! To solve, submit this exact answer to the Game Master: <slug>".
- Answer id `message`, slug `word-word-word-NN`.
- Pitfall: 3-digit octal tokens *also parse as decimal* (and sometimes pass a naive printable check).
  `solve.py` therefore decodes with every plausible base and keeps the most text-like result (score >= 0.9),
  rather than taking the first base that parses (day-05's approach).

## time_trial (DONE, CORRECT - flag fcae423d...) / mastery (DONE, CORRECT - flag 7497b08f...)
- Deadlines: time_trial 6s, mastery 7s (28 sets). 256 ranked attempts in pool; 255 left after each solve.
- One process only: `run_timed.py <time_trial|mastery> [--retry]` (`set -a; . ./.env; set +a`, then `python3 -I`).
  Observed wall time: time_trial 5.7s (download 3.1s), mastery 6.0s (28 concurrent downloads 4.0s) - tight,
  just under the deadline. If one expires, `--retry` (variants are plentiful).
- time_trial blob = same format as single solve (octal list).
- mastery: `start` returned `downloads` (28 per-question URLs, `blob-NN.txt`), answer ids `setN.message`
  <-> `blob-NN.txt` (route by the question's `artifact`, not by id prefix - see memory mastery-id-prefix-routing).
  Each blob = 6 rows `NN<TAB>fragment` of the same sentence, sliced raggedly, encodings per row:
  1 base64, 2 rot13, 3 hex, 4 8-bit binary groups, 5 decimal, 6 **octal**. Decode each row (autodetected by
  `decode_row`, which reuses `decode_numbers` base scoring) and concatenate, then regex the slug.
- `solve.py` is a superset of day-05's solver (verified identical output on all 23 day-05 mastery blobs).
- `run_timed.py` dumps every fetched variant + answers to `artifacts/<mode>-<attempt>/` *after* submitting (the kept examples are `artifacts/{time_trial,mastery}_example_variant/`).
- Likely future days: another numeric base/encoding layer on the same template. Add the new row encoding to
  `decode_row` and re-run the day-05/06 fixtures offline before any timed start.
