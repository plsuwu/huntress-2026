# Day 26 warmups - Food for Thought

Encoding: **Brainfuck** ("A very small language has quite a lot to say."). The single-solve/TT blob is one line of
`+`/`-` runs, each followed by `.`, on a single cell (no `<>[],`, no newline): each run is the delta from the previous
character. Mastery row 26 is the same per fragment, and each fragment is its own program, starting from cell 0:
`26\t+++...+.------.---...` (128 rows, only `+-.` seen). 128 mastery sets (5N-2).

Code: `codec::brainfuck`, a full bounded interpreter (all eight commands, wrapping 8-bit cells, 30000-cell tape, `,`
reads 0, 50M step budget, UTF-8 output). It is `None` unless the text holds only commands and whitespace, has at least
one `.` and one of `+<>[`, and the brackets balance. The `+<>[` rule keeps Morse (`.-` only) from reading as a program.
Lane: `T::Bf`, `fixed_codec(26)`, `structured()` tries it right after code points (before Morse), and `solve_blob`
peels it right after code points. Unit test `brainfuck_blob`.

The first pass refused ("no slug after peeling") and submitted nothing. After the fix (2026-10-26 ~13:02Z),
single-solve (`happy-comet-blossom-79`), regular time trial and regular mastery were all CORRECT on the first submit.
TT/mastery are dual-pool; Hall of Fame is the coordinator's.
Fixtures: `ctf-run/fixtures/warmups/d26-{single_solve,time_trial,mastery}.json`; replay 75/75 clean.
Run dirs: `ctf-run/runs-d26/warmups-day-26/` (`single_solve-7f8005bf-1793019731`, `time_trial-8f39216b-1793019743`,
`mastery-5a5e7452-1793019752`).

Day 27 guess: another esoteric or "small language" notation (Ook!, a Brainfuck that uses loops `[>+++<-]`, which the
interpreter already handles, or whitespace language), or another per-character notation (quoted-printable `=4E`, `0x4E`
tokens, `\N{...}`). Ook! and whitespace are not decoded yet.
