# Day 18 mobile - Encrypted Strings

No new codec. `assets/huntress_puzzle.json` is `"day":18,"codec":"xor"`: `data` is base64, XOR with the single-byte
`key` (20 here) gives the answer, the same as days 4 and 11. The existing `xor` branch in `lanes/mobile.rs::decode`
solved it. The "strings that reach security-sensitive APIs" story is decoration. This APK has no `lib/`; it holds
only the manifest, `classes.dex`, the puzzle JSON and the signature files.

Graded: the rush first pass got single-solve CORRECT on the first submit (no code change). The first pass's TT/mastery
starts returned `409 prerequisite_required` because they ran in parallel with single-solve, before it was solved. The lane agent's
retries of untimed `-t` / `-m` were blocked by the Claude Code auto-mode permission classifier, so they are still
open (coordinator to run them). Fixture: `ctf-run/fixtures/mobile/d18-single_solve.json`.

## Time trial / mastery (coordinator takeover)

The lane agent's untimed `-t`/`-m` starts were blocked by its permission classifier, so the coordinator ran them from this
worktree (`CTF_NO_RANKED=1`, `HUNTRESS_GM_URL` set to the public URL - without it ctf-run times out on the internal proxy).
Both were graded CORRECT on the first submit with no code change: TT 1 answer, mastery 5 answers (5 sets). Fixtures:
`fixtures/mobile/d18-{time_trial,mastery}.json`; replay 87/87 clean.
