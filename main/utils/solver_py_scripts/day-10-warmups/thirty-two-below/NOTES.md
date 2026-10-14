# Day 10 warmups - Thirty-Two Below (`warmups-day-10`)

Same slug-in-encoded-text template as day-05..09; the layer is **base32** (RFC 4648, `A-Z2-7`, `=` padding).
Decoded text: "Nice work decoding this warmup! ... submit this exact answer ...: <slug>".

## single_solve (DONE, CORRECT - flag 682cb1fc...)
- `artifacts/single_solve-blob.txt`, answer id `message`, slug `groovy-starfish-willow-93`.
- `solve.py` = day-09 superset + `decode_b32` (whole-blob layer in `solve`, and in `decode_row` after hex, before base64).
- Route has `run_for_time_supported: false` (older route: timed by default). time_trial deadline 12s (pool 256, 1 set),
  mastery 11s (48 sets, pool 256).
- `run_timed.py` copied from day-09 (`python3 -I run_timed.py <time_trial|mastery> --challenge warmups-day-10 [--retry] [--untimed]`).

## time_trial (DONE, CORRECT - flag cf235a68...) / mastery (DONE, CORRECT - flag a29c95b3...)
- `run_timed.py <mode> --challenge warmups-day-10 --untimed`: **`--untimed` is required** - this route is
  `run_for_time_supported: false`, so sending `run_for_time:true` returns HTTP 409. Plain start is the timed attempt.
- time_trial 4.8s total (download 2.9s, deadline 12s), mastery 5.0s (48 downloads 2.9s, deadline 11s). 255 attempts left
  after each; no retries burned.
- Offline check before the mastery run: synthetic rows (base32 padded/unpadded/lowercase, base64, rot13, hex, binary) all
  decode via `decode_row`. Mastery rows are base32 among the earlier codecs; the base32 check sits after hex, before base64.
- Variants dumped under `artifacts/time_trial-*/` and `artifacts/mastery-*/`.
