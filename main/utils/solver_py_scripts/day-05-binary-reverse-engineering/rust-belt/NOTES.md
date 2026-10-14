# Rust Belt (day 05, binary-reverse-engineering-day-05-rust-runtime-shapes)

Status: single_solve, time_trial, mastery all CORRECT (2026-10-14). Solver: `solve.py <evidence_dir>` -> answers JSON.
Flags: single `61e119ae87b7106a1b02a2f7bb8d958e`, time_trial `6e125886d57680098dabadd462b4009f`, mastery `27ae0d69d884fd28e2ee9504b5528bef`.

## Layout
Scripts + this file at top level; everything produced lives under `artifacts/`.
- `solve.py <evidence_dir>`  all answers from evidence bytes (static only; objdump disassembles, binaries never run)
- `live_run.sh [start|retry] [time_trial]`  one-process start -> download -> 7z -> solve -> submit (writes `artifacts/<mode>/run_<ts>/`)
- `mastery_run.sh [start|practice]`  one-process start -> download deliveries -> `mastery.py` -> submit
- `mastery.py <run_dir>`  `deliveryN.zip` -> `answers.json` keyed `setN.<id>` (same as go-fish)
- Token: `set -a; . ../../../../.env; set +a` (HUNTRESS_GM_TOKEN) before the shell scripts.

## What this challenge is
Exactly the day-04 go-fish question shape (see `../../day-04-binary-reverse-engineering/go-fish/NOTES.md` for grading
rules: 0x-prefixed hex, compact JSON, `PE32_dotnet`, etc.) plus a `d05` block: sha256, architecture, format, HCFG config
campaign/domain/mutex/url/crc32 (40 answers/set). Despite the "Rust" theme no Rust-specific parsing was needed.
`solve.py` = go-fish solve.py + d05 + a hardened `hcfg()`.

## The one new gotcha: decoy 'HCFG' bytes
The Rust d05 binary (4 MB, with debug_info) contains `HCFG` at 3 places (code immediate bytes, the rodata string
`HCFGZerokind__ZN...`, and the real blob). `find(b'HCFG')` first hit is wrong -> crc assertion failed.
`hcfg()` now iterates all hits and accepts the first with ver==1, crc32(plaintext)==header crc, and valid JSON.
d05 key was 0x61 (ver 01, key, len LE16 0x170, crc LE32, xor'd JSON), same layout as d02/d04.

## Mode behaviour (identical to go-fish)
- time_trial: 7 s deadline, 255 retries; a single-process run (~4 s) succeeded on the first `start`. Hall of fame eligible.
- mastery: 7 s deadline, `unique_attempts_remaining: 0`, not HoF-eligible; the one-process `mastery_run.sh start` won it
  within the timer (no `practice` needed this time). Fallback: `mastery_run.sh practice` after expiry.
- Mastery deliveries (7z `infected`, not python zipfile): set1 = d01/d02 ELF, d03 variant-001, d04 `variant-006.elf`, d05 `variant-007.elf`;
  set2 = d02 `.exe` (PE), d03 variant-000, d04 `.exe` (Go PE), d05 `variant-002.elf`. The time_trial variant matched set1's file mix.
  solve.py handled all of them unchanged; d01 was always ELF64 x86-64 EXEC little-endian (assumed, not verified for other variants).
- Not yet seen (would need solve.py changes): d05 as PE (`machine()` already handles PE), d01 ELF32/big-endian.
