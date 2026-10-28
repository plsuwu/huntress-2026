# Day 28 warmups - Back in Sixteen

Encoding: **reversed hex** ("Our sixteen-fingered friend is walking backwards."). The single-solve/TT blob (220 B, no
newline) is the lowercase hex of the sentence, reversed character-wise (so each byte's nibbles are swapped too:
`...e696e4` = "Nin" read backwards). Mastery row 28 is the same per fragment (`28\t7373d227` = `r-ss`), each
fragment reversed on its own. 138 mastery sets (5N-2).

Code: `T::RevHex` (`hex(reverse(t))`), `fixed_codec(28)`; `structured()` tries it after plain hex on an even hex run;
`solve_blob` gained a generic "structural layer written backwards" peel (reversed text through Hex/B64/B32) after the
forward peels fail and before the englishness letter fallback. No `codec.rs` change. Unit test `reversed_hex_blob`.

The first pass refused ("no slug after peeling"), nothing submitted. After the fix (2026-10-28 ~13:02Z) single-solve
(`speedy-button-nugget-52`), regular time trial and regular mastery were all CORRECT on the first submit.
TT/mastery are dual-pool; Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d28-{single_solve,time_trial,mastery}.json`; replay 81/81 clean.
Run dirs: `ctf-run/runs-d28/warmups-day-28/` (`single_solve-6816bcc6-1793192523`, `time_trial-516e3942-1793192533`,
`mastery-6f46a351-1793192543`).

Day 29 guess: another "familiar layer, twisted" (reversed base64/base32 are already peeled by the backwards pass;
nibble-swapped hex without reversal, hex of UTF-16 reversed, octal/binary reversed are not), or the long-predicted
whitespace language / quoted-printable / `0x4E` tokens, none of which are decoded yet.
