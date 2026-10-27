# Day 27 warmups - Monkey Business

Encoding: **Ook!** ("Ook. Ook? Ook! Someone seems chatty."). The single-solve/TT blob (~34 KB) is space-separated
`Ook.`/`Ook!` tokens, read in pairs as Brainfuck: the same one-cell `+-.` delta program as Day 26
(`Ook. Ook.` +, `Ook! Ook!` -, `Ook! Ook.` .; no `Ook?` seen, so no `<>[]`). Mastery row 27 is the same per
fragment, each its own program from cell 0 (`27\tOok. Ook. ...`). 133 mastery sets (5N-2).

Code: `codec::ook` (whitespace optional, `ook` any case, all eight pairs mapped: `. ?` > `? .` < `. .` + `! !` -
`! .` . `. !` , `! ?` [ `? !` ]; `None` unless the text is an even number of tokens with no `? ?` pair), which hands
the program to `codec::brainfuck`. Lane: `T::Ook`, `fixed_codec(27)`, `structured()` tries it before Brainfuck/Morse,
and `solve_blob` peels it **before** the letter/rail passes (on the 34 KB blob those passes cost ~30 ms; now ~1 ms).
Unit test `ook_blob`.

The first pass refused ("no slug after peeling"), nothing submitted. After the fix (2026-10-27 ~13:02Z) single-solve
(`hearty-castle-fern-71`), regular time trial and regular mastery were all CORRECT on the first submit.
TT/mastery are dual-pool (256 ranked / 128 regular); Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d27-{single_solve,time_trial,mastery}.json`; replay 78/78 clean.
Run dirs: `ctf-run/runs-d27/warmups-day-27/` (`single_solve-10e0240f-1793106149`, `time_trial-6da7d639-1793106165`,
`mastery-62c316c1-1793106175`).

Day 28 guess: another esoteric language (whitespace, Malbolge-lite, `Ook` variants like Blub/"Pikalang", a Brainfuck
with real loops/`>`), or a per-character notation (quoted-printable `=4E`, `0x4E` tokens, `\N{...}`). Whitespace
language, QP and `0x` tokens are not decoded yet. Brainfuck token-substitution dialects (any 8 distinct tokens) would be
a cheap generalisation of `codec::ook`.
