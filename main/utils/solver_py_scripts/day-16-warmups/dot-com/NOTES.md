# Day 16 warmups - Dot Com

Encoding: **Morse code of the hex digits** ("Sixteen little symbols are calling. Dot dot dot."): the sentence's UTF-8
bytes as uppercase hex (`4E 69 63 65` = "Nice"), each hex digit sent as one Morse code, separated by single spaces,
with no word separators. So the decode is Morse -> hex -> text. Solved natively in ctf-run with no Python step.

- `codec::morse` / `codec::is_morse` (additive): ITU letters, digits, punctuation; words split on `/`, `|` or 3+ spaces;
  `None` on any unknown code.
- `lanes/warmups.rs`: new `T::Morse`. `morse_variants` tries the hex reading first, then the raw (uppercase) and
  lowercased Morse text, so plain-text Morse would also work. Blob peel tries Morse first among structural layers.
  `structured()` routes any `.-`-only row to Morse.
- Mastery row 16 is pinned to `T::Morse` in `fixed_codec`. It is also Morse of hex, e.g. `16\t-.... .---- --... -----`
  (`6170` = "ap"). 78 sets (5N-2).
- Test gotcha: `rows_with_unknown_caesar_row_learn_across_sets` used row 16 as its "unpinned" row. It now uses row 99.

Graded (untimed, 2026-10-16): single-solve, regular time trial and regular mastery were all CORRECT on the first try.
Route: TT/mastery dual-pool (ranked 256 / regular 128; TT 30 s, mastery 95 s). Fixtures:
`ctf-run/fixtures/warmups/d16-{single_solve,time_trial,mastery}.json`. Run dirs: `ctf-run/runs-d16/warmups-day-16/`.
The first-pass single-solve refusal (before the fix) is in `ctf-run/runs-d16` in the main checkout.
