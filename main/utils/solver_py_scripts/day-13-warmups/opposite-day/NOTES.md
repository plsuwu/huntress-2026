# Day 13 warmups - Opposite Day (`warmups-day-13`)

Slug-in-encoded-text template again; the layer is **Atbash** (a<->z). Decoded: "Nice work decoding this warmup! ... submit this exact answer ...: <slug>".

## single_solve (DONE, CORRECT - flag 3980077e...)
- `artifacts/blob_single.bin`, answer id `message`, slug `handy-candle-quartz-31`.
- `solve.py` = day-11 superset + `decode_atbash` (applied only when result has more English keywords/slugs than input; also in mastery `decode_row` before rot13 fallback).
- Route `run_for_time_supported: true`: plain start = regular untimed; `run_for_time:true` = timed. time_trial deadline 30s (solve budget 1s), mastery 80s (63 sets, budget 63s).
- `python3 -I run_timed.py <mode> --challenge warmups-day-13 [--untimed] [--retry]`

## time_trial (DONE - regular flag 13e3e09e..., timed run CORRECT + hall_of_fame_eligible, 6.4s of 30s)
## mastery (DONE - regular CORRECT, flag 790fb6ac...; timed run CORRECT + hall_of_fame_eligible, 6.5s of 80s)
- 254 unique timed attempts left afterwards; no retries burned beyond the two timed runs.
- Mastery blobs are `NN<TAB>fragment` rows; per-row-index encoding is FIXED across all 63 sets:
  01 b64, 02 **rot13**, 03 hex, 04 binary, 05 decimal, 06 octal, 07 %-hex, 08 &#dec;, 09 "\uXXXX", 10 b32,
  11 b64url (unpadded), 12 **reversed**, 13 **atbash** (digits/hyphens untouched). Single/time-trial = whole-blob Atbash.
- Gotchas hit: (1) letter-only fragments are ambiguous between raw/rot13/atbash/reversed and any combo satisfies the
  template -> use the fixed index mapping first, enumeration only as fallback (`solve_rows`);
  (2) even-length all-digit row 03 can be *hex* (e.g. `6720746869...`) not decimal - digit-only rows only count as
  decimal if space-separated or odd length.
- First mastery attempt (untimed) submitted wrong (old solver) - harmless on the regular route; `start` reconnects and
  resubmission was accepted. Fixtures in `artifacts/mastery-*/` (63 blobs) + `time_trial-*/`.
- Offline regression: all 63 saved mastery fixtures decode to valid slugs before the timed runs.
