# Day 18 script analysis - Process Story ("Orphan Process")

Single-solve CORRECT untimed after two lane fixes (1194 answers, +55 vs day 17). Time trial / mastery were **not run
by the lane agent**: the first pass got 409 `prerequisite_required` (started before single-solve was graded), and the
agent's untimed `-t`/`-m` starts were blocked by the Claude Code auto-mode classifier. They need running by the
coordinator. Fixture: `ctf-run/fixtures/script/d18-single_solve.json`.

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
