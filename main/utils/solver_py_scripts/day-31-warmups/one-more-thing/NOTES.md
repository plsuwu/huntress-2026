# Day 31 warmups - One More Thing

Encoding: **base64 of gzip of base32** ("Thirty-two, a little pressure, then sixty-four. Unpack that thought.").
The single-solve/TT blob is `H4sI...` (base64) -> gzip -> base32 text (`JZUWGZJA...======`) -> the sentence.
Mastery row 31 is the same per fragment (`31\tH4sIAAAAAAACA/MLDA9x93O0BQDqTkLNCAAAAA==` -> `NQWTGNA=` -> `l-3`).
153 mastery sets (5N-2).

Single-solve and TT needed no change (the blob loop peels Gz, then B32). Mastery was refused by the first pass:
row 31 had no fixed codec, `structured()` stopped after `T::Gz` and left base32 text in the sentence
(`...witty-castle-coraNQWTGNA=`).

Code: `T::GzB32` (`base32(gunzip(base64(t)))`), `fixed_codec(31)`, and `structured()` tries `GzB32` before `Gz`/`B64`.

Graded (2026-10-31 ~13:01Z): single-solve `quick-piano-fern-76` CORRECT and regular TT CORRECT (first pass);
regular mastery CORRECT on the first submit after the fix. TT/mastery are dual-pool; Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d31-{single_solve,time_trial,mastery}.json`; replay 90/90 clean.
Run dirs: `ctf-run/runs-d31/warmups-day-31/` (`single_solve-bb00bb77-1793451605`, `time_trial-4e3ffe0d-1793451613`,
`mastery-6f78c749-1793451665`).

Gotcha for future "stacked" days: mastery rows are decoded by one transform only (pinned or `structured()`); a row
whose layer is a composition of earlier layers needs its own `T` variant. A generic fix would be to re-peel a row's
output through `structured()` while it still parses (validated by `consistent`).
