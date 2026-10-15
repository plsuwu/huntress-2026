# Day 15 script analysis - Credential Store Clues ("Crumb Trail")

Cumulative set grew to 1026 answers (single/TT) and 5130 (mastery, 5 sets). New this day:
- `polyglot-perl-*` (4 files) and `host-perl-cgi-perl` (1 file, 11 answers): `my $descriptor_<hex> = "<hex>";` plus the
  `HUNTRESS_ANALYSIS_V1:HEX-...` marker. Already parsed by `families::analyse_literal`; the only fix was routing:
  `family_of` now sends **every `host-` prefix** to the literal family (previously an explicit hta/wsf/php/sct/lua list).
- `collection-*` (T1555.003) / `resource-*` / `impact-*` scenario questions were answered by existing code.

Mastery failure (regular, untimed): only `set5.batch-chain-expansion`. Reconstruction statement
`for %%G in (0 1 2 3) do call set "cfg=%%cfg%%%%h%%G%%"` (fragments named `h0..h3`) is graded **`for_indirect`**; the
Day 3 rule (FOR + `h` prefix -> `call_double`) is wrong for TT/mastery, though the d14 single-solve grader accepted
`call_double` for the same shape. `batch::classify_expansion_v13(raw, single)` is now mode-dependent. `-control` for
`for /l %%Q in (1,1,1) do call :SUB` was accepted as `call_subroutine` (despite the question text's for_once hint).

Localised with `ctf-run/tools/oracle_groups.py` (family-grouped partial submits; ~70 requests) after blind bisection
proved too slow. Route: dual-pool, ranked 32 / regular 32; TT 50 s, mastery 170 s.
