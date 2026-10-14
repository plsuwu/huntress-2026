# Day 6 "Shell We Begin" - status notes (2026-10-14)

Challenge: `script-analysis-day-06-bash-behavior-graphs` (399 answers = Day 5's 357 unchanged + **42 new Bash** answers:
6 specimens x 7 fields). Modes: single_solve, `/time_trial` (7 s, 32 ranked variants), `/mastery` (7 s, 2 sets = 798 ids
`set1.<id>`/`set2.<id>`, one outer download). All three **CORRECT** (single: first submit; TT: 2nd attempt, the 1st was
burned by a solver gap; mastery: first attempt, 0 errors).

## Files
- `shell_we_begin.py` - driver (`--zip|--dir`, `--questions start.json`, `--out`, `--log`). Routes `bash-*` (strips `setN.`)
  to `sh_static.py`; everything else goes to Day 5's `hatch_careful.solve` (-> Day 4 -> Day 3), imported unchanged.
- `sh_static.py` - static Bash analyser; nothing is executed. Parses `Material_<id>` (quoted chunks `'..''..'` are
  concatenated), `Codec_<id>`, `Load_<id>` (representation) and interprets `Decode_<id>` with a tiny statement/arithmetic
  interpreter (ast-whitelisted `$(( ))`; for-loop, `d[len-1-i]=` reversal, `+=()` append, key arrays `local -a key=(..)` with
  `% ${#key[@]}`, scalar `previous=`). Decode pass count comes from `Recover_` (`for x in 1; do Decode_`, `while ((z < N))`).
  The `Codec_` label is only cross-checked (mismatches land in `notes`).
- `live_run.py` - runner (`time_trial|mastery|single_solve --op start|retry|practice`); run with
  `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py ...`, token from env.
- `tests/test_regression.py` - single-solve, time-trial-1-burned (answers = solver output, never graded), time-trial-2,
  plus a synthetic mastery layout (`setN.` ids + `evidence-0N.zip ->` artifacts). `python3 -I tests/test_regression.py`.
- `fixtures/`: `single-solve/` (start, evidence.zip, extracted/, answers, submit-result, evidence-log), `time-trial-1-burned/`,
  `time-trial-2/`, `mastery-1/` (outer zip, start.json, answers.json, result.json). `ex/` dirs are `7z x -pinfected` extracts.

## Bash specimen anatomy
```
readonly Material_<id>='<encoded descriptor>'          # sometimes split in adjacent '..''..' chunks
readonly Codec_<id>='<representation>|<transform>|<key>'
Load_<id>() / Decode_<id>() / Recover_<id>() / Inspect_<id>() / Plan_<id>() ; if [[ HUNTRESS_INSPECT_ONLY ]] ...
```
- Descriptor is Day 3's pipe format: `v=1|steps=a,b|adapter=..|stage_count=N|campaign_id=..|<url|file_path|host_id|cron_path|..>`.
- Representations seen: decimal_space, decimal_colon, reversed_decimal, hex_compact (`${raw:i:2}`), percent_hex,
  base64_literal, base64_chunks. Transforms seen: identity, single_byte_xor, reverse_then_xor, add_modulo_256 (decoder
  *subtracts* k), repeating_key_xor (key literal inside Decode_). Files can carry `# configuration reconstruction` /
  `# reconstruction complete` comments, blank-line padding and indented bodies (all decoys).
- Specimens/roles (same set every variant): host-context=benign_utility (`none`), memory-stage=stager (T1105,
  `dispatch_pipe`), embedded-write=dropper (T1140), remote-write=downloader (T1105), local-dispatch=launcher (T1059.004),
  staged-chain=staged_chain (T1059.004; `profile_host,acquire_remote,write_file,change_mode,dispatch_process`).
  Day 3's `ep.role_of`/`ep.attack_of` work unchanged with language technique **T1059.004** (Unix Shell).
- New question kinds: `-stage-count` = descriptor `stage_count` (**1 for every 2-step specimen, 5 for the chain** - not
  `len(steps)`; accepted by the grader) and `-steps` = descriptor `steps` joined with `>`. `-adapter` = descriptor `adapter`
  (the Plan_ command arrays are not needed). `-host-id/-url/-file-path/-cron-path` = descriptor keys, hyphens -> underscores.
- Adapters seen: readonly_configuration, wget_stdout_pipe, busybox_wget_stdout, printf_decode_redirect, wget_output,
  sh_script, bash_script, wget_arch_fallback, multi_utility_fallback.

## Lessons / risks
- **TT attempt 1 burned** (385/399 accepted, `remaining: 14`): variant 1 had `hex_compact` and `repeating_key_xor`, which the
  first regex-based decoder rejected (`ShUnsupported`). `time-trial-1-burned/` is what the interpreter was rebuilt against.
  The single-solve variant alone does NOT cover the decoder vocabulary - run the solver on every saved fixture
  (`tests/test_regression.py`) before a timed start and prefer an *interpreter* over per-codec regexes.
- Still unseen (guessed, untested): `hex_colon/hex_dash/decimal_pipe/decimal_dash` loads (generic IFS split), `rolling_xor`
  (VBS semantic `out[i]=in[i]^in[i-1]`; a `previous` scalar is supported by the statement interpreter), `while`/`until`
  decode loops, inner `if` in Decode_ (raises `ShUnsupported`).
- Latency (client): start 4.1-5.8 s for a 259 KB (TT) / 532 KB (mastery) body, download 1.5 s, submit 3.8-5 s; the server
  stamps receipt, the solve landed ~2 s into the 7 s window for both TT-2 and mastery.
- Rust port (`ctf-run`) not started for Day 6.
