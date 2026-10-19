# Day 19 warmups - Little Things

Encoding: **hex of UTF-16LE with a BOM** ("Two bytes walk into a bar. The little one goes first."): the blob is
`fffe4e00690063...` (436 B, lowercase hex, no newline), i.e. `FF FE` + the sentence as UTF-16LE. The first pass
REFUSED (`no slug after peeling`): `T::Hex` decoded the bytes, but they failed `printable` (NUL bytes), so the
peel stopped. Solved natively in ctf-run, with no Python step.

- `codec::utf16_text` / `codec::text_any` (additive): a BOM picks the byte order. Without a BOM, every high (or low)
  byte must be zero and the other one non-zero (Latin-1 in either order). `text_any` = UTF-8 `printable`, else UTF-16.
- `lanes/warmups.rs`: `T::Hex`, `T::B64`, `T::B32` and `T::Bin` now decode through `text_any`, so a later
  "<codec> of UTF-16" works with no code change. Mastery row 19 is pinned to `T::Hex`.
- Mastery row 19 carries **its own BOM on every row**: `19\tfffe70006c0065002d0031003700` ("ple-17"). 93 sets (5N-2).
- A unit test covers the blob and a row with and without a BOM.

Graded (untimed, 2026-10-19, about 2 min after the fix): single-solve (`jaunty-raven-glade-73`), regular time trial
and regular mastery were all CORRECT on the first submit. Route: TT/mastery are dual-pool (ranked 256, regular 127
remaining), so Hall of Fame can still be run. Fixtures: `ctf-run/fixtures/warmups/d19-{single_solve,time_trial,mastery}.json`.
Run dirs: `ctf-run/runs-d19/warmups-day-19/`.
