# Day 31 script analysis - XSLT Final Gauntlet

Solved by `ctf-run` with **no lane change**. The automated first pass got all three modes CORRECT: single-solve,
then untimed regular time trial + mastery. Answers: 1846 single-solve (+44 vs day 30), 1950 time trial (+52 vs
day 30's 1898), 29250 mastery (**15 sets** of 1950). Run dirs:
`ctf-run/runs-d31/script-analysis-day-31-xslt-final-gauntlet/` (main checkout, first pass). Fixtures:
`ctf-run/fixtures/script/d31-{single_solve,time_trial,mastery}.json`. Replay 93/93 clean.

New this day: `polyglot-xslt-{benign-utility,downloader,launcher,staged-chain}` (4 x 11, the usual fields).
**No new `host-` file** (+44, not +55).

- `.xsl` polyglots: an XML comment `<!-- HUNTRESS_ANALYSIS_V1:HEX-<payload> -->` on line 2, then an
  `xsl:stylesheet` whose `match="/"` template emits `<descriptor>PAYLOAD</descriptor>` as element text (no
  `descriptor_x = "..."` assignment). The `HUNTRESS_ANALYSIS_V1` marker fallback in `analyse_literal` finds it
  (TT layered `LYR-hex_rev-...` included); the `>...<` generic-literal scan would also catch the element text.
- TT/mastery stay on the day-23 schema (hex-id files such as `3f7778.xsl`, `<family>-<hex>-<kind>` ids, 13 per file).
