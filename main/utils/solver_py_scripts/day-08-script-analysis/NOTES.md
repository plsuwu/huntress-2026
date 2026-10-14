# Day 8 "Lost in Translation" (script-analysis-day-08-cross-language-iocs) - notes (2026-10-14)

Single solve **CORRECT** on 1st full submit: 615 answers = Day 7's 469 (unchanged, `../day-07-script-analysis/node_by_node.py`)
+ **88 `polyglot-php-*`** (8 specimens x 11) + **44 `host-{hta,wsf,php}-*`** (4 specimens x 11) + **14 advanced Bash**
(new Material forms). Driver: `lost_in_translation.py` (`--dir|--zip`, `--questions start.json`, `--out`, `--log`).

## New anatomy
- Descriptor is a string literal `descriptor_<id> = "..."` (PHP `$descriptor_`, JS `var/const`, VBS plain). Decoded it is
  `v=3|language|role|verdict|attack|adapter|host_profile|obfuscation_profile|stage_profile|next_language|stage_count|steps|campaign_id|url|file_path`.
  Encodings (auto-detected, nested): plain hex, fragmented hex (`.` separators), base64, hex-of-base64.
  The `HUNTRESS_ANALYSIS_V1:<KIND>-...` marker comment duplicates the literal - ignored.
- **Answers are the descriptor's own fields** (role, attack, adapter, host_profile, obfuscation_profile, stage_profile,
  next_language, stage_count, steps joined `>`, campaign_id, url, file_path). Do NOT derive role/attack from `steps`: here
  steps correlate with obfuscation (plain_hex->decode_configuration,return_configuration etc.) and `ep.role_of` raises/disagrees,
  but the grader accepted the descriptor values (e.g. uploader/T1041, persistence_descriptor/**T1053**, launcher/T1059).
- Advanced Bash new forms: `hex_compact` pair loop (`$((16#${raw:i:2}))`), `decimal_csv` (`IFS=','` + `printf '\\x%02x'`).
- Fixtures: `fixtures/single-solve/{start.json,evidence.zip,ex/,answers.json,log.json,submit-result.json}`.

## Status (all three modes CORRECT, regular/untimed attempts; `run_for_time_supported: true`)
- single_solve flag b800c109bc95748d3791222d3c5a9113; time_trial 812c005dc5efc899b7300500b6d20f08 (615 answers);
  mastery a1e197937eeee8b1e8e2554568e1fca8 (3075 answers = 5 sets x 615, one outer zip; `setN.` prefix handled by the
  stripping in `lost_in_translation.answer_ml` / Day 7 routers). 0 solver errors on mastery.
- TT regular variant added a 3rd advanced-Bash form: `xor_hex` (`Codec_=xor_hex|3e`, Decode `$((16#.. ^ 16#3e))`, key in
  Decode body) - handled in `analyse_adv_bash` (xor key regex). Single-solve alone doesn't cover it; first TT submit was
  partial (5 remaining) - harmless untimed.
- Timed (`--timed`, ranked deadline 9 s TT / 22 s mastery) NOT run: 12-min driver budget. `ranked_attempts_remaining` 32.
  Latency client-side: start 4.4-6 s (371 KB TT, 1.9 MB mastery), submit 3-4 s - a 9 s TT window is very tight; dry-run on
  `fixtures/{single-solve,time-trial-regular,mastery-regular}` first (`live_run.py ... --dry`).
- `live_run.py` = Day 7's with import/CHALLENGE swapped; run via
  `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py <mode> --op start [--out DIR]`.
  Download member has no `.zip` suffix (`1-<name>`); rename before `7z x -pinfected`.
- Not done: Rust port, regression test script.
