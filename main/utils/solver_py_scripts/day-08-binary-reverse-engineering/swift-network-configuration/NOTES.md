# Swift Justice (day 08, binary-reverse-engineering-day-08-swift-network-configuration)

Status: single_solve CORRECT first try (2026-10-14), flag `08a828dd29458e1eef019bae07aad040`. Static analysis only; nothing delivered was executed.
time_trial CORRECT on 2nd timed attempt, flag `e6fe37db3ad90d63cd8402ceaebc83c5` (HoF eligible). mastery CORRECT first timed attempt (2026-10-14), flag `ae95c0a32f1753fb781e762060e166e7` (HoF eligible, 31 attempts left).

## Layout
- `solve.py <evidence_dir>`  all answers from evidence bytes -> JSON (= day-07 routine-check solve.py + d08 + newly asked d01-d07 fields)
- `live_run.sh [start|retry] [time_trial|mastery]`  one-process start -> download -> 7z -> solve -> submit (`artifacts/<mode>/run_<ts>/`). Needs `HUNTRESS_GM_TOKEN`.
- `artifacts/single_solve/` start.json, evidence.zip, ev/, answers.json, submit_body.json, result.json

## Question shape
80 questions = 10 per day d01..d08. d08 is a 67 MB Swift ELF (x86-64, dynamically linked, not stripped).
d08: `artifact_sha256_1`, `analysis_architecture`, `analysis_machine` (`x86-64`), `analysis_format` (`ELF`),
`analysis_elf_class` (**integer `64`**, not `ELF64` as in d01), `analysis_config_{campaign,domain,mutex,url}`, `analysis_config_crc32_hex`.
The Swift "networking/URL configuration" is just the usual HCFG blob (`HCFG|ver=1|key|len LE16|crc32 LE32|payload^key`, JSON payload),
stored as a base64 string constant (starts `SENGR...`) like the d06/d07 JARs. The raw `HCFG` bytes found in the ELF (offset ~90031, code bytes) are a decoy:
crc32 check rejects it. No Swift metadata parsing needed. Use `data.find(b'SENGR')` not a regex scan (67 MB; whole solve 0.14 s).

## New d01-d07 fields (first asked on day 08; not in day-07 solver)
`d01_analysis_entry_point` (e_entry, 0x-hex), `d01_analysis_has_symtab` (true/false), `d02_analysis_has_cpp_{typeinfo,vtable}` (true/false; `_ZTI<n>`/`_ZTV<n>` symbols),
`d03_analysis_config_{crc32_hex,url}` (d03 dll carries a raw HCFG blob, key 0x41), `d0{4,5}_analysis_hcfg_version` (header byte `1`), `d0{4..7}_analysis_xor_key_hex` (0x-hex key byte),
`d0{6,7}_analysis_compression_methods` (distinct ZIP methods of all entries, JSON `[0, 8]`). All accepted in single_solve.

## Mode behaviour (the real difficulty is bandwidth, not analysis)
- time_trial: deadline **10 s**, pool 113. The delivery is a 31 MB AES zip (d08 ELF = 67 MB, 25 MB packed); solving takes 0.14 s, 7z 0.25 s,
  but the download from this environment runs 3-5 MB/s (6.7-9.5 s measured; parallel downloads share the same pipe, `Range` is ignored -> full 200).
  Attempt 1 EXPIRED (12.7 s wall: download 9.5 s) and burned a variant; attempt 2 (`live_run.sh retry time_trial`, 11.1 s wall incl. start) was CORRECT.
  Success is luck of link speed: just `retry` and race; 112 -> 111 attempts remaining after the two. `op=artifact` and `op=download` return identical bytes.
- mastery: deadline **27 s**, pool 32, 5 sets; nested zips would be ~5 x 31 MB (~150 MB) = ~30+ s at observed 4-5 MB/s, i.e. not reachable from this link.
  NOT started (driver wall-clock budget exhausted and a start burns a variant). Next agent: try from a faster link/VPS (the proxy
  `10.0.0.219` route in HOMEBASE.md), or `practice` after a burned start; answers come from `mastery.py` pattern in day-07 routine-check
  (keys `setN.<id>`), solve.py here already handles d01-d08 per directory.
- A faster pipeline idea if bandwidth stays the limit: stream-decrypt the zip while downloading (WinZip AES) so only the last 25 MB member is waited on; saves <1 s.

## mastery: CORRECT (2026-10-14) - flag `ae95c0a32f1753fb781e762060e166e7`, hall_of_fame_eligible, 18.3 s wall of 27 s
- `mastery_run.py [start|retry|practice]` (run with `python3 -I`, token from env): start -> 5 parallel streaming downloads
  (5 x ~31.5 MB, bare AES zips, `downloads[]` in the start response) -> per-delivery 7z extract + `solve.py` as each lands ->
  one batched submit; keys `setN.<id>` with N = delivery order (400 answers = 5 x 80, 0 missing/extra vs question ids).
  `--local <zip>` runs extract+solve offline (0.4 s per delivery) - use it as the regression check before a timed start.
- Timing this session (link ~9.5 MB/s): start 3.0 s, downloads finished 11.4-14.8 s, solve <0.4 s each, submit 3.1 s.
  The earlier "not reachable from this link" estimate (4-5 MB/s) was too pessimistic: parallel streams aggregated
  ~10 MB/s. If the link is slow, a `start` that expires burns 1 of 32 variants; just `retry`.
- Fixture: `artifacts/mastery/run_*/` (start.json, delivery1-5.zip, answers.json, result.json).
