# Day 24 warmups - String Theory

Encoding: **run-length pairs** `count:char,` ("Count carefully. The comma knows where to stop."). The single-solve/TT
blob is one line `1:N,1:i,1:c,1:e,1: ,...,1:7,1:6,` that expands straight to the fixed sentence. Every count seen so
far is `1` (single, TT and all 590 pairs of mastery row 24); the decoder handles any count anyway. Mastery row 24 is
the same per fragment: `24\t1:e,1:l,1:-,1:6,1:5,` (= `el-65`). 118 mastery sets (5N-2).

Gotchas: the pair's character is exactly one char and may itself be `,` (`1:,,`), `:` (`1::,`), a space (`1: ,`) or
any escape-looking byte, so parsing is positional (digits, `:`, one char, `,`-or-end), never a split on `,`. The comma
after the last pair is present in the blobs; the decoder treats it as optional and ignores trailing whitespace.

Code: `codec::run_length` (`None` unless the whole text parses). Lane: `T::Rle` (applied untrimmed: a leading or
trailing pair may be a space), `fixed_codec(24)`, `structured()` tries it right after delimited Ascii85, and
`solve_blob` peels it before Ascii85 and the escape/percent/entity peels. Unit test `run_length_blob`.

The first pass refused ("no slug after peeling") - nothing was submitted. After the fix (2026-10-24 ~13:42Z): single-solve
(`jaunty-compass-mosaic-76`), regular time trial, regular mastery all CORRECT on the first submit. TT/mastery are
dual-pool (256 ranked / 128 regular, mastery deadline 135 s); Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d24-{single_solve,time_trial,mastery}.json`; replay 69/69 clean.
Run dirs: `ctf-run/runs-d24/warmups-day-24/` (`single_solve-b3e3d6bb-1792849341`, `time_trial-fc414a81-1792849351`,
`mastery-19a65167-1792849362`).

Day 25 guess: another "counting"/structural wrapper - RLE with real runs (counts > 1), a different pair syntax
(`char:count`, `Nx`, `3a2b`), or a layered combination (RLE of hex/base64). A digit-first compact RLE (`3a2b`) would
not parse here and needs its own decoder, template-checked.
