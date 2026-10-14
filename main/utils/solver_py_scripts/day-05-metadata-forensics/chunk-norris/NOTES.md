# Chunk Norris (day 05, metadata-forensics)
Challenge id: `metadata-forensics-day-05-chunk-norris` (+ `/time_trial`, `/mastery`). Prompt: "walk PNG chunks, verify
declared geometry, find the image with an unexpected structural passenger".

## Status: all three modes CORRECT
| mode | flag | question(s) seen |
| --- | --- | --- |
| single_solve | 6ae2fedaa360de16c29fa74653a9d67d | `d05-png-001-png-ihdr-dimensions-pixels` -> `269x153` |
| time_trial | f9ce7f9c5e137ab5a97cb80169699c09 | `d05-png-002-...` same kind (1 question) |
| mastery | 9d9984ac8dd879a7e373e7968aa09a4c | `set1.d05-png-003-...`, `set2.d05-png-004-...` same kind |

## Observations
- Only kind seen: `png.ihdr_dimensions_pixels`, answer format `<W>x<H>` (IHDR width/height, big-endian u32 at offset 16).
  Same shape as day-04 Picture This (`WxH`). The "unexpected passenger" / geometry-mismatch hint never appeared as a question
  in the variants I saw (single zip holds one tiny 414-byte PNG: IHDR, IDAT, IEND, no extras).
- Question ids embed a per-variant counter (`d05-png-NNN`) and filename (`panel-NNN.png`): always take ids from the response.
- Zip is not password protected; single/time-trial download is a plain zip, mastery returned a (possibly nested) zip -
  `nested_zip.py` handles both. Mastery set N <-> `metadata-day-05-evidence-0N.zip` -> `panel-*.png`.
- Time trial and mastery deadlines are 10s; whole loop runs in one process (~1s). First `start` of a timed mode begins
  the clock, so `solve.py <mode> --submit` goes straight to solving (no exploratory start needed).
- Run: `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} [--retry] [--submit]`.

## If later variants add kinds (as day-04 mastery did)
`solve.py` dispatches on question-id suffix via `HANDLERS`; unknown kinds print `UNHANDLED` plus a `FACTS` dump
(IHDR tuple, chunk-type list, bad-CRC chunks, trailing-bytes length) to stderr and are not submitted.
`chunks()` already walks chunks with CRC validation - extend `HANDLERS` (e.g. chunk list/count, trailing bytes after IEND,
text chunks, bit depth / colour type) rather than rewriting. Latest artifact + start JSON saved in `artifacts/`.
