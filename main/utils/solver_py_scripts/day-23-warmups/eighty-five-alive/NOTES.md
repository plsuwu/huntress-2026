# Day 23 warmups - Eighty-Five Alive

Encoding: **Adobe Ascii85** (`<~ ... ~>`; "The alphabet got bigger. Adobe might recognize the neighbors."). The
single-solve/TT blob is one delimited Ascii85 string that decodes straight to the fixed sentence. Mastery row 23 is the
same per fragment: `23\t<~ATB@[2u~>` (= `er-58`). 113 mastery sets (5N-2).

Code: `codec::ascii85` (delimiters optional, `z` = 4 zero bytes, partial final group padded with `u`) and
`codec::is_ascii85` (delimited only). Lane: `T::A85`, `fixed_codec(23)`, `structured()` checks the delimiters first,
and `solve_blob` peels a delimited Ascii85 blob **before** the other structural peels (the alphabet contains `\`, `%`,
`&`, which the escape/percent/entity peels would otherwise eat).

**Mastery gotcha (latent since day 5, first hit today):** row 05 is decimal, but a fragment made only of digits 0-7
(`116 104 105 115` = "this") is also valid octal ("NDEM"), and `codec::numbers` takes octal on a score tie. 20 of
113 sets hit "no row combination forms the sentence", so the solver refused the first untimed mastery (no
submit, no attempt lost). Fixed by pinning rows by base:
`fixed_codec` 4 -> `T::NumBase(2)`, 5 -> `NumBase(10)`, 6 -> `NumBase(8)` (`codec::numbers_radix`); an unreadable
row still falls back to `structured()` -> `T::Num`.

Graded (untimed, 2026-10-24 ~07:30Z): single-solve (`crisp-starfish-quartz-70`), regular time trial, regular mastery
(second attempt, after the radix fix) all CORRECT. TT/mastery are dual-pool; Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d23-{single_solve,time_trial,mastery}.json`; replay clean.
Run dirs (merged): `ctf-run/runs-d23/warmups-day-23/` (the first mastery, `mastery-9eea579d-1792826937`, was refused by the solver - `errors.json`, nothing submitted;
the CORRECT one `mastery-9eea579d-1792827057`).

Day 24 guess: another "bigger alphabet" (base58, base62, Z85, base91, base45, uuencode/xxencode, yEnc), or Ascii85
without delimiters / btoa-style. Z85 has no delimiters and its alphabet overlaps base64, so it would need
template-checked attempts after the cheap peels.
