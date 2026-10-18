# Day 18 script analysis - Process Story ("Orphan Process")

All three modes CORRECT untimed after two lane fixes. Cumulative set: 1194 answers (single/TT, +55 vs day 17), 5970
(mastery, 5 sets). The first pass got 409 `prerequisite_required` on TT/mastery (started before single-solve was graded).
Both passed on the first untimed regular attempt after single-solve was CORRECT. Route: dual-pool, ranked 32 / regular 32.
Fixtures: `ctf-run/fixtures/script/d18-{single_solve,time_trial,mastery}.json`.

New this day (55 answers): `polyglot-google_apps_script-{benign-utility,downloader,launcher,staged-chain}` (`.gs`, 4 x 11)
and `host-google-docs-sidebar-google-apps-script` (11, `host_profile=google_docs_sidebar`).

1. **Split literal.** The descriptor is no longer one quoted token. It is a JS array of string fragments joined at
   runtime: `const HUNTRESS_ANALYSIS_V1_SPLIT_<id> = ["FRAG-..", ...].join("")` (marker copy, first fragment prefixed
   `FRAG-`/`B64-`), and `encoded_<id> = [..].join("")` inside `buildAnalysisPlan_<id>()`. Encodings are the usual
   dotted-hex / base64. `families::analyse_literal` now joins every run of `"..."` separated by `,`/`+`, strips a
   leading `KIND-` prefix, and decodes. It needs `steps` and exactly one distinct result.
   Trap: on the staged-chain file the old quoted-token fallback accepted a **single base64 fragment** that decoded
   to a truncated `v=3|language=..|role=staged_c..` (it has `role`, so `ok` passed). The joined step now runs first.
   Unit test: `split_fragment_literal` (`testdata/d18-polyglot-gas-staged-chain.gs`).
2. **Batch expansion (graded).** `batch-downloader-expansion` (`remote-write.bat`, `for %%G in (0 1 2 3) do call set
   "cfg=%%cfg%%%%h%%G%%"`) was graded `for_indirect` on **single-solve**. Days 14-17 single-solve accepted
   `call_double`. TT/mastery have wanted `for_indirect` since d15. `classify_expansion` passes `single = day < 18`, so
   older fixtures still replay. The grouped oracle found it in one pass (it was the only failing group).

## 2026-10-18 grader change: batch `expansion` on every day's TT/mastery (coordinator)

During the post-release HoF pass, every ranked script TT/mastery from day 3 on came back `incorrect`. Ranked runs
started at 13:17-13:18Z were still CORRECT on the old labels; every run from 13:42Z failed. Day 1-2 (no Batch) passed.
The grouped oracle on an untimed Day 9 mastery (`ctf-run/runs-d18/regress/`) isolated only the `batch-*-expansion`
fields. Per-field submits showed the GM now labels the reconstruction statement **by its syntax alone**:

| statement | label now | old rule (days <= 11: v3; 12+: v13) |
|---|---|---|
| `for ... in ... do call set "..%%x%%%%n%%G%%"` | `for_indirect` | v3 `call_double` for an `h` prefix |
| `call set "%%a%%%%b%%"` (any names, also with an `if defined` prefix) | `call_double` | `delayed` (v3) / `percent` (v13) unless names were `h<N>` |
| `!a!!b!` (also with an `if defined` prefix) | `delayed` | v13: `if defined` -> `percent` |
| `%a%%b%` (also with an `if defined` prefix) | `percent` | v3 `delayed` |

`batch::classify_expansion_syntax` implements it. `classify_expansion` uses it for TT/mastery attempts whose server
`started_at` is at or after `SYNTAX_RULE_FROM_MS` (13:30Z). `RunCtx.started_ms` is set from the start response, and in
replay from the fixture's `start.json`. Older fixtures keep their old graded labels. Confirmed: Day 9 mastery CORRECT
(untimed, via partial submits), Day 18 mastery CORRECT (untimed, 5970 answers), then ranked Day 3 mastery CORRECT.
Single solves keep the per-day rules: they were all solved before the change and were not re-checked.
`tools/exp_statements.py RUN_DIR EXTRACT_DIR VERDICTS` prints each expansion question's `set` statements with an
oracle verdict (extract the delivery with `7z x -pinfected`; Python's zipfile can't read its compression method).
