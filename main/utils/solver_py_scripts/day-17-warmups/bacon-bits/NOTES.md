# Day 17 warmups - Bacon Bits

Encoding: **Bacon-style 5-bit a/b groups of the hex digits** ("Sixteen guests. Five little a's and b's apiece."):
the sentence's UTF-8 bytes as hex (`4E` = "N"), each hex digit written as 5 bits, `a` = 0 and `b` = 1, MSB first,
one space between groups (`aabaa abbba` = `4E`). So the decode is Bacon -> hex -> text. The values never go above 15,
so a classic 24/26-letter Bacon reading gives junk (`EO...`). Solved natively in ctf-run with no Python step.

- `codec::is_bacon` / `bacon_values` / `bacon_letters` (additive): a/b in either case, groups whitespace-separated
  or run together (every group must be a multiple of 5 long).
- `lanes/warmups.rs`: new `T::Bacon`. `bacon_variants` tries the hex reading first (both polarities), then classic
  26- and 24-letter (I=J, U=V) Bacon text, lowercased and uppercase. Blob peel tries Bacon right after Morse.
  `structured()` routes any a/b-only row to Bacon.
- Mastery row 17 is pinned to `T::Bacon` in `fixed_codec`. It is Bacon of hex too, e.g.
  `17\taabba aaaab aabbb aaaaa ...` (`6170...` = "ap..."). 83 sets (5N-2).
- First pass: the single-solve was REFUSED (`no slug after peeling`). TT/mastery returned 409 `prerequisite_required`
  because single-solve was not solved yet, so on this route TT/mastery unlock only after the single-solve is CORRECT.

Graded (untimed, 2026-10-17): single-solve (`tidy-rocket-harbor-20`), regular time trial and regular mastery were all
CORRECT on the first submit after the fix. Route: TT/mastery dual-pool (ranked 256 / regular 128 remaining).
Fixtures: `ctf-run/fixtures/warmups/d17-{single_solve,time_trial,mastery}.json`. Run dirs: `ctf-run/runs-d17/warmups-day-17/`.
