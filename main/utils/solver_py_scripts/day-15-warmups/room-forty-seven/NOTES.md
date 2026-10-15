# Day 15 warmups - Room Forty-Seven

Encoding: **ROT47** ("Even the punctuation went for a spin"): every printable ASCII `!`..`~` rotates by 47, so digits
and the slug's hyphens change too (`-` <-> `\`). Implemented natively in ctf-run (`codec::rot47`, `T::Rot47`); no
Python reference this day.

Gotcha: ROT47 output contains backslashes (`-c` -> `\4`), and the blob peeler tried backslash-escape decoding first,
turning `\42` (`-c`) into `"` -> `sunnyQstle-...`. `solve_blob` now checks every letter/rotation transform for a template
match *before* any structural layer.

Mastery (73 sets = 5N-2) row 15 is pinned to ROT47 in `fixed_codec`. Route config: TT/mastery dual-pool
(ranked 256 / regular 128; TT 30 s, mastery 90 s). All three modes CORRECT first try (untimed regular attempts).
Fixtures: `ctf-run/fixtures/warmups/d15-{single_solve,time_trial,mastery}.json`.
