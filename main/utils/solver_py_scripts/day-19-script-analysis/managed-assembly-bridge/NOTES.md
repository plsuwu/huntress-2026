# Day 19 script analysis - Managed Assembly Bridge

Solved by `ctf-run` with **no lane change**. The automated first pass (chained) got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Cumulative set: 1249 answers (single/TT, +55 vs day 18), 6245 (mastery,
5 sets). Run dirs: `ctf-run/runs-d19/script-analysis-day-19-managed-assembly-bridge/` (main checkout, first pass).
Fixtures: `ctf-run/fixtures/script/d19-{single_solve,time_trial,mastery}.json`.

New this day (55 answers): `polyglot-csharp_script-{benign-utility,downloader,launcher,staged-chain}` (`.csx`, 4 x 11)
and `host-aspx-inline-csharp-csharp-script` (11, `host_profile=aspx_inline_csharp`). Same 11 fields as day 18.

- **Polyglot `.csx`**: plain C# `string descriptor_<hex> = "<hex-of-base64>";` plus the usual
  `// HUNTRESS_ANALYSIS_V1:HB64-...` marker. The first `analyse_literal` pattern matches it directly.
- **Host `.aspx`** (named `.csx`): `<%@ Page Language="C#" %><script runat="server">` holding the whole C# script as one
  escaped string, `EmbeddedCsx_<hex> = "// HUNTRESS_ANALYSIS_V1:HEX-...\n...descriptor_<hex> = \"...\"..."`. The
  `descriptor_` pattern does **not** match the escaped `\"`. The descriptor comes from the `HEX-` marker fallback.
  It was correct, but it depends on the marker. Hardening idea (not done, the brief said bookkeeping only): allow an
  optional `\\` before the quotes in the first pattern, and add a no-marker unit test like `literal_without_marker`.
