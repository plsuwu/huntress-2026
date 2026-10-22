# Day 22 warmups - Deflated Ego

Encoding: **base64 of a zlib stream** ("Another familiar wrapper. This one needs a little inflation."), the zlib twin
of Day 21's gzip. The single-solve blob is `eNol...` (zlib header `78 da`, deflate, adler32 trailer) and inflates
straight to the fixed sentence. Mastery row 22 is the same wrapper per fragment: `22\teNpLLdI1MwcABPABcg==`.
108 mastery sets (5N-2).

**No code change was needed**: Day 21's `decompress` already accepts a valid zlib `78xx` header, so `T::Gz` and the
base64-looking-row path in `structured()` solved all three modes on the automated first pass's first submit.
Afterwards, mastery row 22 was pinned to `T::Gz` in `fixed_codec` (row-N convention only).

Graded (untimed, 2026-10-22 ~15:00Z, first pass): single-solve (`tidy-cloud-tulip-70`), regular time trial and
regular mastery were all CORRECT. TT/mastery are dual-pool, so Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d22-{single_solve,time_trial,mastery}.json`; replay 63/63 clean.
Run dirs: `2026/ctf-run/runs-d22/warmups-day-22/`.

Day 23 guess: another compression wrapper (raw deflate with no header, bzip2 `BZh`, xz `fd 37 7a`, zstd `28 b5 2f fd`,
lzma), or a wrapper inside a different text layer (hex/base32 of gzip/zlib). Raw deflate has no signature, so it would
need an explicit inflate attempt after the header checks fail; the others need a new magic in `decompress`.
