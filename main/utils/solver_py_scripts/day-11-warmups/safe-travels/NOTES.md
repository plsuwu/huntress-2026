# Day 11 warmups - Safe Travels (`warmups-day-11`)

Same slug-in-encoded-text template as day-05..10; the layer is **base64url** ("URL-friendly base": `-`/`_`, padding possibly stripped).
Decoded text: "Nice work decoding this warmup! ... submit this exact answer ...: <slug>".

## single_solve (DONE, CORRECT - flag 294d4793...)
- `artifacts/single_solve-blob.txt`, answer id `message`, slug `crisp-starfish-petal-89` (this variant was plain std base64).
- `solve.py` = day-10 superset + `decode_b64` (std + url-safe alphabets, padded/unpadded) used both as whole-blob layer and in `decode_row`.
- Route `run_for_time_supported: false` -> timed by default; use `run_timed.py <mode> --challenge warmups-day-11 --untimed`.
  time_trial deadline 10s (pool 256), mastery 8s (53 sets, pool 256).

## time_trial (DONE, CORRECT - flag 10b9182b...) / mastery (DONE, CORRECT - flag 2fcff2ce...)
- `python3 -I run_timed.py <time_trial|mastery> --challenge warmups-day-11 --untimed` (`--untimed` required: route is
  `run_for_time_supported: false`, `run_for_time:true` -> HTTP 409; plain start is the timed attempt).
- time_trial 5.0s total (download 2.85s, deadline 10s); mastery 6.9s (53 downloads 3.5s, deadline 8s). 255 attempts left
  after each; no retries burned.
- Before mastery: regression-ran new `solve.py` over all day-10 fixtures (49/49 identical answers). `decode_b64` min length
  lowered to 4 so short mastery row fragments don't fall through to the rot13 fallback.
- Variants dumped under `artifacts/time_trial-*/` and `artifacts/mastery-*/`.
