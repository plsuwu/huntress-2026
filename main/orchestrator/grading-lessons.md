# Grading lessons

Game Master and grader behaviour that cost attempts, and the procedure that came out of each case. Per-challenge
details live in `utils/solver_py_scripts/day-NN-<lane>/<slug>/NOTES.md`.

## Localising wrong answers

- **Partial submits are an oracle on untimed attempts.** On a timed TT/mastery attempt, a wrong full submit returns
  only `{"result":"incorrect"}`. On an untimed attempt, submitting a *subset* returns `accepted` + `accepted_ids` +
  `remaining`. A wrong subset returns `incorrect`, while correct fields are banked.
  - After fresh timed attempts run out, `{"op":"practice","challenge":"<id>/<mode>"}` gives an untimed continuation on
    the last artifact, where the oracle works.
  - Space single-field submits about 3.5 s apart.
  - Re-derive each wrong field from evidence; don't cycle through values.
  - Day 3 "Expansion Pack" was solved this way.
- **Group by family first.** `ctf-run/tools/oracle_groups.py RUN_DIR` submits once per question family
  (`<lang>-<family>`, or binary roll-ups grouped by day `d01`..`dNN`, about 17 submits). It then splits rejected groups
  by field. One wrong field kind is the usual failure mode. On Day 15 this found the single bad field among 5130
  script-mastery answers, where blind bisection (serial 2-6 s submits) was too slow.
- **A wrong field masks the others in a partial submit,** so probe label vocabularies one field per submit.
- **Single-solve resubmits are free.** Test an alternative format with one curl submit before encoding it.

## Before spending a timed attempt

- **Save first.** Save every timed artifact to disk before solving, solve offline, then finish via practice/oracle.
- **Strip the set prefix.** Mastery answer ids are `set1.<id>`, `set2.<id>`, ... A prefix or suffix test on the
  whole id silently misroutes every question, so strip `^set\d+\.` before routing.
  - Day 5 "Hatch Careful": the first mastery attempt was burned this way (252 answers unsolved, then `expired`).
  - Always replay the previous mastery fixture offline before a timed start.
- **Single-solve undersamples the vocabulary.** Day 6 Bash: single-solve used 5 representation/transform combos, but
  TT variant 1 added `hex_compact` + `repeating_key_xor`. The regex decoder raised an error and the attempt was burned
  (385/399).
  - For a new language, build the decoder as a small whitelisted interpreter of the function bodies, not one regex
    per codec.
  - Pre-support every codec seen earlier: hex_colon/dash, decimal_pipe/dash, rolling_xor, repeating_key_xor,
    reversed_*.
  - Treat the first timed start as a likely fixture-collection run.
- **Keep whitespace verbatim.** Day 12 "Back at You" was one-shot: the single timed mastery came back `incorrect`
  because the row solver `.strip()`ped a rot13 row that began with a space. Row 12 was also `reverse(frag)`
  right-padded with `-` to width 13. It was recovered via `op=practice`.
  - Keep row whitespace verbatim: strip only `\r\n`.
  - Resolve dash runs against the full message template, requiring a unique hit.
  - On pool-1 routes, the first timed start is the only fixture-collection run.
- **Prerequisites (since Day 17):** TT/mastery answer `409 prerequisite_required` until single-solve is CORRECT.
  Binary-RE mastery also waits for TT. A release probe must treat `409 start_required` as released.

## The GM re-grades without notice

On 2026-10-18 at about 13:30Z, the script-analysis **Batch expansion** label was re-graded on every day's TT/mastery.
A solver that was 100% on fixtures then failed 7 ranked runs in a row.

- **Current rule:** the label is the reconstruction statement's own syntax.

  | Syntax | Label |
  |---|---|
  | FOR-indirect | `for_indirect` |
  | `call set "%%a%%.."` | `call_double` (any names) |
  | `!a!` | `delayed` |
  | plain `%a%` | `percent` |

  An inline `if defined` prefix doesn't matter.
  - Code: `batch::classify_expansion_syntax`, gated in `lanes/script_analysis.rs::classify_expansion` on
    `RunCtx.started_ms >= SYNTAX_RULE_FROM_MS`.
  - Confirmed on untimed Day 9 and Day 18 masteries.
- **Older eras are kept so old fixtures replay:**
  - Days 3-11 used v3: percent shapes gave `delayed`.
  - Day 12+ TT/mastery used v13: inline `if defined` gave `percent`; call set with `h<N>` names gave `call_double`;
    everything else gave `percent`.
  - Day 15 mastery graded FOR-over-`h<N>` as `for_indirect`.
  - Single-solve graders differed per day: d13 = v3, d14-17 accepted `call_double` for FOR-over-h.
  - Details: `utils/solver_py_scripts/day-18-script-analysis/process-story/NOTES.md`.
- **Procedure:**
  1. Stop that lane's campaign.
  2. Reproduce on an untimed regular variant: `--retry`, without `--timed`.
  3. Run `oracle_groups.py`.
  4. Gate the new rule on attempt start time (`RunCtx.started_ms`), not on day, so old fixtures stay valid.
- **Old ledgers are hints, not truth.** The question contract itself can change between sessions. Old ledgers
  (`~/Documents/.workdir/3/...`) are hints only.

## Label vocabularies the grader picks from the generator

The Day 17 binary roll-up (d01..d17 in one evidence.zip) rejected about 150 labels before the hits:

- **`dNN_analysis_hcfg_storage`:** the value is `<encoding>_<where found>`.
  - `base64`: a base64 string constant (d08/d09/d11/d13).
  - `raw_hcfg_section`: a raw XOR blob in a `.hcfg` section (d14).
  - `raw_const_array`: a raw blob in .data, with no section or symbol (d12, Pascal).
- **`d02_analysis_{mangled_score,typeinfo}_symbol`:**
  - ELF: `_ZNK15DiscoveryEngine5scoreEPKc` / `_ZTI15DiscoveryEngine`.
  - MinGW PE (mastery sets): the first COFF symbol in file order, i.e. `.text$_ZNK...` / `.rdata$_ZTI...`.
  - The typeinfo class is the one that has a `_ZTV`; BehaviorEngine is the vtable-less base.
- **Stripped d01:** `recover_code` = `none`.

## Environment

- **Python TLS:** the uv Python interpreter needs `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt` for https.
