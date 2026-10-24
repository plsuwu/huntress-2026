# Day 23 script analysis - Game Resource Loader ("Press Start to Decode")

All three modes CORRECT, untimed. The automated first pass got single-solve CORRECT with no change (1461 answers;
new family `polyglot-gdscript-*` `.gd` files, `const DESCRIPTOR_<HEX>: String = "..."`, standard B64/HEX/HB64/FRAG
markers). Time trial (1495) and mastery (10465 answers, **7 sets**) needed lane work. Runs (all merged):
`ctf-run/runs-d23/script-analysis-day-23-game-resource-loader/` (`single_solve-7a0dedeb-*` first pass;
`time_trial-af60e618-*`; `mastery-7fcb7ee8-1792827691`). Fixtures: `fixtures/script/d23-*.json`.
Replay 69/69 clean.

## What was new in time trial / mastery (single solve is still the old schema)

TT/mastery file names are now **hex ids** (`10c7ec.php`), not role names; question ids are `<family>-<hex>-<kind>`.

1. **Layered literals**: the marker is `HUNTRESS_ANALYSIS_V1:LYR-<recipe>-<payload>`, recipe = encode order of
   `hex`, `b32`, `b64`, `rev`, `frag` (1-3 layers, e.g. `hex_rev_hex`, `b64_b32_frag`, `b32_frag_b32`); decode
   in reverse. `frag` (`.` fragments) and `rev` can sit at any layer. `descriptor::decode_literal` now does a
   bounded search (depth 6: hex/base32/base64 per layer, dots stripped, one reversal between decodes) until
   `v=`; it no longer reads the recipe. The old marker regex stopped at the recipe's `-`; it now skips `LYR-<recipe>-`.
   Base32 is new (`codec::base32` already existed).
2. **Composite questions** (`script_analysis.rs::composite`, built from the existing per-field answers):
   - `-campaign-token`: campaign id after the first hyphen (every family, incl. bash-advanced).
   - `-indicator` (JScript/Batch/VBS/Bash/Node/Python): first present of url, upload_url, next_stage, file_path,
     schedule_path (cron_path/task_path), note_name, host_id - straight from the question text.
   - `-style`: JScript `function|invocation|property` style; VBS `representation|transform|control`; Batch
     `graph|control|expansion` (topology = the descriptor `graph`).
   - `-steps` for JScript/Batch/VBS/Python (Bash/Node had it).
   - Python `-carrier` = `carrier|windows_chain`, `-obfuscation` = `syntax|obfuscation` - **only when the question
     text says "joined by |"** (single-solve uses the same suffixes for one label). `answer_spec` now gets the
     question text for this.
   - Polyglot `-url-host` (URL host, no port/userinfo), `-path-leaf` (last `/` or `\` segment of file_path).
   - PowerShell `-secondary` / `-tertiary`: scenario slots. resource hijacking: wallet / mutex; collection:
     **the first collected file** (`update-<id>.dat`, `c[0]`) / user agent; impact: note / extension.
     Graded: the collection "staged file name" is NOT the `stage-<id>.zip` string in the record (tried it,
     incorrect). Collection records also carry a `Global\` mutex now (unused).

## Gotchas

- `-function-style` etc. also end in `-style`: the style composite must skip those or it recurses forever (stack
  overflow in replay).
- `tools/oracle_groups.py` groups by the first two id components, which is now per file (`python-afd564`), so the
  TT oracle ran ~1500 submits (~6 min). The last line's `remaining` told the whole story (26 = 13 python x 2).
  Don't pipe it through `tail` - nothing shows until it exits.
