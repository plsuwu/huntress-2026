# Day 17 script analysis - Download Graph ("Fetch Quest")

Solved by `ctf-run` with **no lane change**. Single-solve passed on the automated first pass. Time trial and mastery
failed to start on that pass with HTTP 409 `prerequisite_required`, because they were started before single-solve was
graded. Re-run untimed afterwards, both were CORRECT. Cumulative set: 1139 answers (single/TT), 5695 (mastery, 5 sets).
Route: dual-pool, ranked 32 / regular 32; TT 50 s, mastery 170 s.
Fixtures: `ctf-run/fixtures/script/d17-{single_solve,time_trial,mastery}.json`.

New this day (44 answers): `polyglot-auto_hotkey-{benign-utility,downloader,launcher,staged-chain}` (`.ahk`, 4 x 11).
**No `host-` file this day.** The prompt's "download graph / network evidence" narrative did not become a question.
The literal is AutoHotkey v2 `descriptor_<hex> := "<hex-of-base64>"`. The assignment regex did not accept `:=`, so the
descriptor was found only through the `; HUNTRESS_ANALYSIS_V1:HB64-...` marker fallback (same as Ruby on day 16).

Hardening (in `families::analyse_literal`): the first pattern now accepts `:=` and matches case-insensitively
(Ruby `DESCRIPTOR_<HEX>`), so neither depends on the marker. The unit test `literal_without_marker` uses
`testdata/d17-polyglot-ahk-*` and adds a decoy literal so the quoted-token fallback can't pass it by accident.

Gotcha for the rush first pass: TT/mastery `start` returns 409 `prerequisite_required` until single-solve is CORRECT.
Starting all three at once costs nothing, but TT/mastery must be retried after single-solve.
