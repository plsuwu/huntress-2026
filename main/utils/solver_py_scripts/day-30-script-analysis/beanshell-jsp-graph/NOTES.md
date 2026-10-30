# Day 30 script analysis - BeanShell JSP Graph

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Answers: 1802 single-solve (+55 vs day 29), 1898 time trial (+65 vs
day 29's 1833), 26572 mastery (**14 sets** of 1898). Run dirs:
`ctf-run/runs-d30/script-analysis-day-30-beanshell-jsp-graph/` (main checkout, first pass). Fixtures:
`ctf-run/fixtures/script/d30-{single_solve,time_trial,mastery}.json`. Replay 90/90 clean.

New this day: `polyglot-beanshell-{benign-utility,downloader,launcher,staged-chain}` (4 x 11, the usual fields) and,
for the first time since day 25, **a new `host-` file**: `host-jsp-beanshell-beanshell` (11), so +55, not +44.

- `.bsh` polyglots: `// HUNTRESS_ANALYSIS_V1:FRAG-<dotted hex>` marker on line 1, then
  `String descriptor_<hex> = "<dotted hex>";` and a `Map buildPlan_<hex>()` that wraps it. The Java-style typed
  declaration matches the primary `descriptor_\w+ = "..."` pattern in `analyse_literal`.
- `host-jsp-beanshell-beanshell.bsh` is actually a JSP page (`<%@ page import="bsh.Interpreter" %>`) whose
  `<%! private static final String INERT_BEANSHELL_<HEX> = "..." %>` holds a whole BeanShell script (with the
  marker) as an escaped string. The `HUNTRESS_ANALYSIS_V1` marker fallback finds it. It is routed as a literal
  by the `host-` prefix (it is not in `ML_PREFIXES`, but `family_of`/`guess_family` accept any `host-`).
- TT/mastery stay on the day-23 schema (hex-id files such as `736ea1.bsh`, `<family>-<hex>-<kind>` ids, 13 per
  file).
