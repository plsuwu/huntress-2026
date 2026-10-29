# Day 29 warmups - Rotten Base

Encoding: **ROT13 over base64** ("Sixty-four invited thirteen over. Things got scrambled."). The single-solve/TT blob
(148 B) is the base64 of the sentence with ROT13 applied to its letters (`GzywMFO3...` = rot13 of `TmljZSB3...`);
digits, `+/=` untouched. Mastery row 29 is the same per fragment (`29\tMF04Aj==` = rot13 -> `ZS04Nw==` = `e-87`),
each fragment padded base64 on its own. 143 mastery sets (5N-2).

Code: `T::RotB64` (`base64(rot13(t))`), `fixed_codec(29)`; `solve_blob` gained a generic "structural layer under a
letter transform" peel (Rot47/Atbash/Rot1-25 each through B64/B32/Hex, first printable decode wins) after the forward
peels and before the reversed peel. The shift is not hard-coded. No `codec.rs` change. Unit test `rot13_base64_blob`.

Gotcha: before the fix, the englishness fallback rotated the raw blob into `FyxvLEN3...` (a different shift) and the
pass ended with "no slug after peeling" - nothing was submitted.

After the fix (2026-10-29 ~13:01Z) single-solve (`jovial-comet-nugget-31`), regular time trial and regular mastery
were all CORRECT on the first submit. TT/mastery are dual-pool (256 ranked / 128 regular); Hall of Fame is the
coordinator's. Fixtures: `ctf-run/fixtures/warmups/d29-{single_solve,time_trial,mastery}.json`; replay 84/84 clean.
Run dirs: `ctf-run/runs-d29/warmups-day-29/` (`single_solve-beb37595-1793278881`, `time_trial-bddbe6bb-1793278894`,
`mastery-33558921-1793278909`).

Day 30 guess: another "familiar layer, twisted" combo (ROT13/atbash over base32 or hex letters is already peeled by
the new pass; base64 of reversed text, XOR-hex, Vigenere-over-base64 are not), or the long-predicted whitespace
language / quoted-printable / `0x4E` tokens, none of which are decoded yet.
