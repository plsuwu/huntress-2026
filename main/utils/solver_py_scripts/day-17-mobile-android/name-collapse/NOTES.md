# Day 17 mobile - Name Collapse

New codec `"indexed"`: `data` is the answer's 20 characters permuted, with an integer `order` array of the same
length; `data[i]` belongs at position `order[i]` (scatter, same convention as day 5/10 `split`). The gather direction
(`out[i] = data[order[i]]`) does not validate. The "identifier shrinking and decoy insertion" story is decoration
(classes.dex only holds strings like `indexed` / `answer`).

Fix: `lanes/mobile.rs::decode` gets an `indexed` branch; the generic search also tries scatter and gather whenever the
puzzle has an `order` permutation matching the current buffer's length, so a renamed permutation codec is found too.

Graded: single-solve, untimed TT (one d17 APK) and untimed mastery all CORRECT first submit after the fix. Mastery (5 APKs)
sampled d4 xor(key 104, base64), d8 reverse, d12 hex, d14 caesar(key 14), d17 indexed. Route: dual-pool, ranked 32 /
regular 32; TT/mastery need single-solve solved first (`409 prerequisite_required` on the rush first pass, which starts
all three at once). Fixtures: `ctf-run/fixtures/mobile/d17-{single_solve,time_trial,mastery}.json`.
