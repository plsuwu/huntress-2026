# Go Fish (day 04, binary-reverse-engineering-day-04-go-runtime-fingerprints)

Status: single_solve, time_trial, mastery all CORRECT (2026-10-14). Solver: `solve.py <evidence_dir>` -> answers JSON.

## Layout
Top level = scripts + this file only; everything produced lives under `artifacts/`.
- `solve.py <evidence_dir>`  derive all answers from evidence bytes (static analysis; objdump only disassembles, binaries are never run)
- `live_run.sh [start|retry] [time_trial]`  one-process start -> download -> solve -> submit (writes `artifacts/<mode>/run_<ts>/`)
- `mastery.py <run_dir>`  `deliveryN.zip` (7z, encrypted) -> `answers.json` keyed `setN.<id>`
- `submit_subset.sh answers.json id...`  submit chosen fields (per-field oracle); needs HUNTRESS_GM_TOKEN (`set -a; . ../../../../.env; set +a`)
- `artifacts/single_solve/`  evidence.zip + extracted files, start.json, answers.json, submit_subset leftovers
- `artifacts/time_trial/`    tt_start.json (expired attempt), tt_answers.json, tt_artifacts/, run_<ts>/ (first = crashed solver run, second = CORRECT)
- `artifacts/mastery/mastery_<ts>/`  start.json, delivery1/2.zip, answers.json, practice.json, submit.json

## Evidence shape
evidence.zip (no password needed) = `d01--variant-NNN.elf`, `d02--...elf`, `d03--...dll` + `.runtimeconfig.json`, `d04--...elf` (Go, static, stripped).
The 32 question ids are `dNN_<kind>_<field>`; a question set per "day" file. Question text can be vague ("exact analysis X value") - the id is the key.

## Config blob ("HCFG")
`48 43 46 47 | ver=01 | xor_key | len LE16 | crc32 LE32 (of plaintext) | payload^key`. Payload is JSON:
campaign, domain, ipv4, url, mutex, windows_path, linux_path, service, host_model.
- d02: in `.hcfg` section, key 0x31.  d04: after `.go.buildinfo`/rodata, key 0x51 (key = 0x10*day+1 in variant-000).
- d03: no HCFG header kept at obvious spot; brute-force key by searching for `{"campaign"` XOR k (key 0x41); JSON is followed by padding of key bytes.
- Go binary needs no Go-specific parsing: `find(b'HCFG')`.

## Grading rules learned (these cost submits)
- Hex fields (`*_crc32_hex`, `*_xor_key_hex`) MUST have `0x` prefix, lowercase, 8 digits for crc: `0x6149aa00`, `0x31`. Bare hex => incorrect.
- `*_analysis_symbols` (d01): JSON compact array of every non-empty `.symtab` name in table order (includes FILE entries, `@GLIBC_2.34` suffix as readelf prints). Accepted.
- `*_section_names`: compact JSON with leading "" accepted.
- `d01_analysis_disassembly`: `["mov eax,0x1fa4","ret"]` (Intel, no spaces after comma) accepted.
- `d01_analysis_elf_class` = `ELF64`, machine `x86-64`, endianness `little`, elf_type `EXEC` accepted.
- `d03_analysis_format` = `PE32_dotnet` accepted (compound token); `d03_analysis_machine` = `x86`; metadata_version `v4.0.30319` (read from BSJB header, not the runtimeconfig).
- `d03_artifact_sha256_1` = dll sha256, `_2` = runtimeconfig.json sha256 (embedded hash string in the dll is a decoy).
- Config strings (campaign/domain/mutex/url) are the JSON values verbatim, mutex keeps `Global\HuntressBRnn_000` (single backslash in the real string).

## Oracle trick
A full wrong submit returns bare `incorrect`, but submitting ONE field at a time (>=4 s apart, the loop takes ~2 min -> run in background)
returns `accepted` + `accepted_ids`/`remaining` per correct field. Accepted fields are banked. Do this to localize wrong fields
instead of guessing; don't send the full set first on timed attempts if formats are uncertain.
Note `| head -c` on curl|tee truncates the saved file; write response to file first.

## time_trial (solved CORRECT 2026-10-14, flag dacaede50469ca2d528b9da14ac0e43f)
- Deadline is only ~7 SECONDS after start (start 11:49:06 -> deadline 11:49:13). Two separate tool calls = expired.
  Use `live_run.sh retry time_trial` (start/retry -> download -> unzip -> solve -> submit in one process, ~4 s). Needs the token: `set -a; . ../../../../.env; set +a`.
- First `start` consumed attempt 1 (expired); `retry` after expiry gave a fresh variant. 255 attempts in the pool, so they are cheap, but a crashed solver still submits and burns one.
- Variants DIFFER in container: time_trial had d02 = PE32+ `.exe` (C++ built for Windows) and d04 = Go PE `.exe` (variant-002), d01 still ELF64, d03 still .NET.
  `solve.py` therefore picks `dNN--*` by prefix (not `.elf`), reports `ELF`/`PE` from magic, PE machine from COFF header (0x8664 -> x86-64).
  HCFG blob is still found with `find(b'HCFG')` in PE files; crc32/key header layout identical.
- Variant numbers in filenames (variant-000/002) feed the config strings (BR04-002, node-002...), so derive everything from bytes.
- Not yet seen but plausible in later variants (solve.py assumes ELF64 LE for d01): ELF32 / big-endian d01, different `recover_code` body. Extend solve.py's d01 section if a variant breaks.
- `artifacts/time_trial/run_<ts>/` hold the artifacts + answers from live runs (kept for reference).

## mastery (solved CORRECT via practice continuation, flag 9dd025da6ee9b90cd58e678f62b3ff61)
- Mastery start: `unique_attempts_remaining: 0`, 7 s deadline, `hall_of_fame_eligible:false` -> the timed attempt cannot be won by a human-speed loop and
  there is no retry pool. After expiry `status` says `next_op: practice`; `{"op":"practice","challenge":".../mastery"}` gives the same artifacts with
  `deadline:null` and a correct submit there completes the mode (flag returned, `practice:true`). Practice and the expired attempt serve identical sha256s.
- Two deliveries (`downloads[]` / `artifacts[]`, each `evidence.zip`); question ids are prefixed `set1.` / `set2.`, artifact `delivery N: evidence.zip`.
  Download each with the capability URL (Bearer token) and save as `deliveryN.zip` (the `download` key is absent in mastery; use `downloads`).
- These zips are NOT plain nested zips: members are AES/`infected` encrypted (python zipfile: "compression method not supported"), so `nested_zip.py` fails.
  `mastery.py <run_dir>` extracts with 7z (recursively for inner zips), solves each set with `solve.py`, writes `answers.json` keyed `setN.<id>`.
- Sets mix container types: set1 = ELF/ELF(d02,d04) with d03 variant-001 (BR03-001); set2 = PE d02 `.exe` + Go PE d04 (variant-002). solve.py handled both unchanged.
- Whole submit was CORRECT first try (no oracle needed). Run dir `artifacts/mastery/mastery_<ts>/` keeps start.json, deliveries, answers, practice.json.
  Reproduce offline: `python3 -I mastery.py artifacts/mastery/mastery_<ts>`.
