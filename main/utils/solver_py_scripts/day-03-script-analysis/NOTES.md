# Day 3 "Expansion Pack" - status notes (2026-10-14)

Challenge: `script-analysis-day-03-batch-expansion` (159 answers: 3 PowerShell, 8 JScript, 9 Batch specimens).
Files: `expansion_pack.py` (static solver, library + CLI), `live_run.py` (start/retry/practice -> download ->
solve -> submit; needs `SSL_CERT_FILE` for uv pythons and `pyzipper`), `fixtures/` (two time-trial archives +
their start JSON, saved so they can be re-solved offline).

## What is verified
- All three archives seen so far (single-solve archive and the two time-trial variants) decode completely:
  159/159 answers, 0 errors. Interpreter-derived Batch descriptors match the independent
  `HUNTRESS-MATERIAL` blob decode for all 27 batch specimens, so descriptor-level fields
  (adapter / campaign / url / paths / note / graph / steps) are not decoy-affected.
- JScript decoder families seen: +/-k shift, const XOR, in-place indexed XOR, chained (previous-byte) XOR, repeating
  key XOR, reverse (in recover_* and/or in decoder), identity; transports: int array(s) + concat, %xx, base64
  (custom alphabet read from source), `a-b-c` decimal, `aa:bb` hex.

## Final state (2026-10-14)
| Mode | Result |
|---|---|
| Single solve | CORRECT (earlier session; flag in GM list) |
| Time trial | CORRECT via `op=practice` continuation (2 live attempts `incorrect` while the solver was incomplete; 0 fresh attempts left, so not Hall-of-Fame eligible) |
| Mastery | CORRECT on first live retry, 9.2 s of a 13 s deadline (outer zip -> evidence-01.zip / evidence-02.zip, ids `setN.<id>`) |

## What mattered (graded evidence)
- Practice mode banks partial answers (`accepted`, `accepted_ids`, `remaining`): that gave a per-field oracle. 141/141
  non-expansion/control fields were accepted on the first partial submit, then 12/18 of the batch expansion/control
  fields; the 6 failures drove the rules below.
- Control: first control construct executed before the reconstruction: `if errorlevel` guard line (`ver >nul` +
  `if errorlevel 1 exit /b 1`) = `errorlevel_guard` even though it does not syntactically wrap the `set`;
  dispatch `goto` = `goto_dispatch`; `call :LABEL` = `call_subroutine`; `if defined HUNTRESS_READY` = `if_guard`;
  `for /l (1,1,1) do call :LABEL` = `call_subroutine` (`for_once` was REJECTED for both such specimens).
- Expansion (graded, empirical): percent-assembled and delayed-assembled sets are both `delayed` (`percent` was rejected
  every time); FOR-indexed accumulation is `for_indirect`; `call set "x=%%a%%..."` is `call_double` only when the
  fragments are named `h<N>` (otherwise `delayed`); FOR accumulation over `h` fragments was `call_double` in the older
  data.  This is an empirical fit to 18 graded specimens from two grader generations, not a derivation - treat as suspect
  if a new specimen family appears.
- relay specimens are `staged_chain`, ATT&CK language-specific `T1059.003` / `T1059.007` (graded).

## Not done
- Rust port (`ctf-run/src/solvers/batch_expansion.rs` still panics on purpose and encodes the old echo-keyed rules; port
  from `expansion_pack.py`, not from that draft).
- Re-run mastery/time-trial purely for Hall of Fame time (mastery has 30 fresh attempts; time trial has none).

---

# Day 2 -> Day 3: what changed, and how to carry solutions forward

Day 2 = `../day-02-script-analysis/host_with_the_most.py` + `findings/FINAL.md`
(`script-analysis-day-02-encoded-intent`, PowerShell + JScript, 87 fields, 5 s timers).
Day 3 = `expansion_pack.py` here (PowerShell + JScript + **Batch**, 159 fields, 8 s / 13 s timers).

## What changed

| Area | Day 2 solution | Day 3 (what was needed) |
|---|---|---|
| Scope | PS (3) + JS (8 specimens), 87 fields | + 9 Batch specimens, 159 fields; mastery = 2 sets = 318 fields |
| PowerShell fields | keyed by file stem and fixed JSON key letters (`values["b"]`, `values["h"]`) | key letters are shuffled per sample: assign by value *shape* (`https://`, `wallet_`, `Global\`, `Mozilla/`, `.locked`, `.txt`) |
| JS decoder | regexes for fixed names (`keyBytes`, `previous`, `packedText`, `firstPart`, `characters`, `v_a19c`), ops ordered by text position | names are randomised per variant, plus decoy strings and new shapes (in-place `x[i] = x[i] ^ k`, reverse before/after the loop, `a-b-c` / `aa:bb` transports, custom base64). Replaced by: find byte source by *shape*, evaluate the decoder's loop body with a tiny integer-expression evaluator, then verify the result parses as JSON |
| Role / ATT&CK | membership tests over the step list; `T1059.007` hard-coded | terminal-step classification from the rubric text; language technique parameterised (`T1059.003` batch / `.007` JS) |
| Syntax answers | from the `inspect_*` function | same idea, but named function expressions (`var x = function x_body()`) and decoy accessors on other objects needed explicit handling |
| Batch (new) | n/a | a small cmd.exe *model* (percent / FOR-var / delayed / CALL expansion, goto / call / for / if) that runs the `:HUNTRESS_INSPECT` section and records which constructs executed before the reconstruction `set` |
| Mastery layout | one download per set, `set{n}/` prefixing | one outer ZIP holding `evidence-01.zip`/`evidence-02.zip`; ids `setN.<id>`; routed by the archive name in the question's `artifact` label |
| Contract | question text was sufficient | question text was **not** sufficient for 18 batch fields; the grader's labels had to be learned from graded feedback |
| Feedback | single solve gave per-answer `accepted`/`remaining`, used to fix 7 fields | single solve was already spent; timed modes give bare `incorrect`; the oracle was `op=practice` partial submits |
| Transport | internal proxy | external `https://2026.huntress.ctf.games/` + bearer; uv Python needs `SSL_CERT_FILE` |
| Timing budget | 5 s | start ~2-3.5 s, each download ~1.5 s, submit ~1.6-2.7 s on this link; mastery 9.2 s of 13 s. Speed was never the blocker, correctness was |

## What went wrong (so it is not repeated)

1. **Two timed attempts were burned on an incomplete solver.** Both times the artifact was a *new variant*
   (attempt 1: new decoder shapes; attempt 2: in-place indexed XOR). 31 mastery variants exist but time trial had 2.
2. **I treated the question text as the grader's spec.** It was only partly true. `percent` is never an accepted
   expansion label; `for_once` was rejected; guard lines that do not wrap the `set` still count as the control.
   The faithful-reading rule scored 12/18 on the batch expansion/control fields.
3. **Whole-set resubmission is one bit.** Two full resubmits told me nothing; the partial-submit oracle located the
   six failures in one pass.
4. **Descriptor-level checks were not the problem** - the independent `HUNTRESS-MATERIAL` decode agreed with the
   interpreter for 27/27 batch specimens, which usefully ruled out a whole class of suspicion early.

## How future agents should use this

**Reuse as a library, not by copy-paste.**
- Lift `read_zip` / `expand_deliveries` / `pick_delivery`, `analyse_js` + `run_decoder`, `role_of` / `attack_of` and
  `analyse_ps1` into a shared module (e.g. `utils/solver_py_scripts/common/script_analysis.py`) and have each day's
  script import it. Day 2's script was reused here only as *evidence about the decoder vocabulary*; its fixed names and
  key letters would have broken on Day 3's variants, so I rewrote rather than extended it.
- Replace "what is this file called" with "what shape is this value / what does this decoder body compute" everywhere.
  Filenames, key letters, variable names, role names and ids move between variants.

**Before spending any timed attempt:**
1. Solve every archive you already have, and *mutate* them: rename identifiers, reorder fragments, swap codecs,
   add a guard line, stack constructs. Assert the number of cases so a vacuous PASS cannot happen.
2. Keep a **variant corpus** (`fixtures/`) of every artifact ever downloaded plus its start JSON; each new variant
   is a regression test for the next run. The corpus here is 3 archives (1 single-solve, 2 time-trial).
3. Make the live runner write the start JSON and every download to disk *before* solving, and submit whatever was
   derived (partial banking in mastery/practice means a partial submit still earns information).
4. Gate "ready" on: decoded descriptor parses, answer count == `answer_ids` count, nothing unrouted.

**Use graded feedback deliberately.**
- Keep a per-field **ledger**: `{field, value, contract-text hash, artifact sha256, verdict, route}`. Verdicts are only
  comparable inside one contract version - the Day 3 older runs had `exact_text` questions graded on the echo line,
  the current ones are enums with "ignore echo" wording, and the same bytes got different labels.
- Prefer the cheapest oracle: single solve per-answer `accepted` > practice/mastery partial banking > nothing.
  Spend single solve's per-answer feedback on the *judgement* fields (role, ATT&CK, expansion, control), not on
  descriptor strings you can verify yourself.
- Probe in groups first (all non-judgement fields as one request), then singly for the failing group, spaced >= 3.5 s,
  stop on HTTP 429. After a wrong field, re-derive from evidence and state what changed; do not enumerate synonyms.
- When the text-faithful reading disagrees with the grader, trust graded data and write the rule *with its
  evidence table* so the next agent knows how many data points support it (see "Expansion" above: 18 points).

**Anticipate how later days compound.**
- Days stack lanes (Day 3 = Day 2's PS/JS + Batch). Expect Day N+1 to reuse the same specimen names, ids and rubric
  vocabulary (`steps`, roles, `next_stage`) with a new language or obfuscation layer. Keep role/ATT&CK logic
  language-agnostic and put language specifics (technique id, syntax questions, decoder family) behind small adapters.
- Expect "carry findings forward" questions (e.g. `next_stage` naming a file from another lane). Have the solver
  emit a structured per-specimen record (descriptor, role, evidence, provenance) rather than just an answers dict,
  so cross-file questions can be answered from the records.
- Expect contract revisions mid-event. Hash each question's text at solve time and diff against the ledger; a changed
  hash means earlier verdicts for that field no longer transfer.
- Port to Rust (`ctf-run`) only from a *Python reference that has reached CORRECT*, and port the rules plus their
  evidence table, not a draft's guesses. `ctf-run/src/solvers/batch_expansion.rs` predates these findings and encodes
  the old echo-keyed rules; it still panics on purpose.

## Open risks for the next run of this challenge
- The `h<N>` call_double rule (and `percent` never being accepted) is an empirical fit; a new specimen family could
  break it. Re-check with one grouped partial submit in practice/mastery before trusting it.
- Control stacking priority (first construct executed) is consistent with all graded data but never tested with two
  constructs on one specimen.
- Mastery's 30 remaining attempts can be used for Hall-of-Fame time; time trial has none left.
