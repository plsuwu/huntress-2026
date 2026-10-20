# Day 20 warmups - Big Entrance

Encoding: **hex of UTF-16BE with a BOM** ("Same bar. This time the big one goes first."), the big-endian twin of
Day 19: the blob is `feff004e0069...` (440 B, lowercase hex, no newline). Mastery row 20 carries its own BOM on every
row, as row 19 did: `20\tfeff007000650072002d00330035` ("per-35"). 98 sets (5N-2).

**No code change was needed**: Day 19's `codec::text_any` / `utf16_text` already picks the byte order from the BOM.
The automated first pass solved all three modes on its first submit. Afterwards, mastery row 20 was pinned to
`T::Hex` in `fixed_codec`, which only follows the row-N convention (the structural hex path already found it).

Graded (untimed, 2026-10-20 ~13:00Z, first pass): single-solve (`deft-blanket-clover-43`), regular time trial and
regular mastery were all CORRECT. TT/mastery are dual-pool, so Hall of Fame is still open (coordinator).
Fixtures: `ctf-run/fixtures/warmups/d20-{single_solve,time_trial,mastery}.json`.
Run dirs: `2026/ctf-run/runs-d20/warmups-day-20/`.

Day 21 guess: another byte-level text representation (UTF-32, UTF-16 without a BOM, or a UTF-8 BOM), or a new
whole-string cipher. `text_any` covers UTF-16 without a BOM only for Latin-1 text.
