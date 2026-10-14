# Day 7 "Node by Node" (script-analysis-day-07-node-operation-graphs) - notes (2026-10-14)

Single solve **CORRECT** (flag 49a3a334beea9a99f9d6efac49b88b72) on 2nd full submit. 469 answers = Day 6's 399 (unchanged,
solved by `../day-06-script-analysis/shell_we_begin.py`) + **56 Node** (`nodejs-<specimen>-*`, 8 specimens) + **14 advanced
Bash** (`bash-advanced-{indirect,tokens}-*`).

## Files
- `node_static.py` - static Node `.mjs` analyser (`Material_`, `load_`, `decode_` ops read in source order, `recover_` order).
  Reps seen: base64_chunks, base64_literal, hex_buffer, hex_array (JSON of hex strings), escaped_octets (`\\x..`).
  Transforms: reverse_bytes, identity, single_byte_xor, rolling_xor (seed = initial var; out[i]=in[i]^in[i-1]).
  Unseen/guessed: key xor arrays, add/sub mod 256, split-based loads (implemented, untested).
- `node_by_node.py` - driver (`--zip|--dir`, `--questions start.json`, `--out`, `--log`); also handles advanced Bash
  (base64 descriptor `v=1|feature=|primitive=|campaign_id=|stage_count=|steps=`; role `advanced_syntax`, attack T1059.004).
- `fixtures/single-solve/` start.json, evidence.zip, ex/, answers.json, log.json, submit-result.json.

## Node anatomy / answers
- Decoded descriptor is the standard pipe descriptor. Specimens: context=benign_utility(none), memory-stage=stager(T1105),
  embedded-write=dropper(T1140), remote-write=downloader(T1105), outbound-transfer=uploader(T1041), local-dispatch=launcher
  (T1059.007), schedule-descriptor=persistence_descriptor (**T1053.003**), staged-chain=staged_chain (T1059.007).
- New role value `persistence_descriptor`; its ATT&CK answer was NOT T1053.005 (incorrect), T1053.006 (incorrect), T1053
  (incorrect) - **T1053.003** (cron; path `/etc/cron.d/..`) accepted. Found via single-solve partial oracle.
- stage-count: 1 for 2-step graphs, 5 for chain (descriptor `stage_count`).
- Single-solve `submit` of a subset returns `accepted`+`accepted_ids`/`remaining` (partial oracle works untimed too).

## Time trial / mastery (done *untimed*, regular attempts; `run_for_time_supported: true`)
- Plain `start` on `/time_trial` and `/mastery` gives a regular untimed attempt; both **CORRECT** (TT: 469 answers,
  mastery: 938 = `set1.`/`set2.`, one outer zip -> inner zips). No timed (`--timed` / `run_for_time:true`) attempt was run
  (12-min driver budget); `ranked_attempts_remaining` was 32 - a Hall of Fame run is still open: `live_run.py time_trial --op start --timed`
  (7 s window; Python solve ~0.2 s, download+submit ~5 s - tight, so dry-run on fixtures first).
- Regular variants added vocab not in single-solve: Node loads `percent` (`/%([0-9a-f]{2})/`), `\u00xx` (generic
  `matchAll(/re/)` handled); advanced Bash Decode `percent_hex` and `reversed_base64` (Material reversed *before* base64).
  First TT/mastery runs submitted partial banks (`accepted`, `remaining`) - harmless untimed; fixed and re-run `start`.
- Advanced Bash is the only "new" Day 7 Bash content; Node answers share Day 3's descriptor/role logic with T1059.007.
- Fixtures: `fixtures/{single-solve,time-trial-regular,time-trial-regular-2,mastery-regular,mastery-regular-2}`.
- Not done: Rust port; regression test script (run `node_by_node.py --dir/--zip` on fixtures before any timed start).
