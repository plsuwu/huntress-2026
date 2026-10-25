# Day 25 warmups - You Have a Point

Encoding: **Unicode code point notation** `U+004E U+0069 U+0063 ...` ("Every character has a point. A code point,
anyway."). The single-solve/TT blob is one line of space-separated `U+XXXX` tokens (uppercase hex, 4 digits) that
decodes straight to the fixed sentence. Mastery row 25 is the same per fragment: `25\tU+0072 U+006E U+002D U+0036
U+0030` (= `rn-60`); row 25 is the last row of each set and has no trailing newline. 123 mastery sets (5N-2).

Code: `codec::code_points` (`U+`/`u+` then 4-6 hex digits per token; whitespace/commas between tokens optional, since
`U` is not a hex digit the markers delimit tokens; `None` unless the whole text parses; supplementary planes accepted).
Lane: `T::Cp`, `fixed_codec(25)`, `structured()` tries it right after delimited Ascii85, and `solve_blob` peels it
right after run-length (before the escape/percent/entity peels). Unit test `code_point_blob`.

The first pass refused ("no slug after peeling") - nothing was submitted. After the fix (2026-10-25 ~13:02Z):
single-solve (`brave-beacon-wizard-34`), regular time trial, regular mastery all CORRECT on the first submit.
TT/mastery are dual-pool (256 ranked / 128 regular); Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d25-{single_solve,time_trial,mastery}.json`; replay 72/72 clean.
Run dirs: `ctf-run/runs-d25/warmups-day-25/` (`single_solve-65e2119a-1792933334`, `time_trial-80e60a7d-1792933346`,
`mastery-64e92e5b-1792933353`).

Day 26 guess: another per-character notation (`\N{...}` names, `&#x..;` hex entities, `0x4E` tokens, `\x4E`, UTF-8
byte escapes `=4E` quoted-printable, punycode) or a counting/structural wrapper. Quoted-printable and `0x`-tokens are
not peeled yet.
