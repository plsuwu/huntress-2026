# Pool Party (day 06, binary-reverse-engineering-day-06-java-bytecode-loader)

Status: single_solve, time_trial, mastery all CORRECT first try (2026-10-14). Static analysis only; nothing delivered was executed.
Flags: single `8e99d46bc3aecdffac566d72484bb45a`, time_trial `b7a8b0c47841140667615cae37fe9585`, mastery `04d1a0f05e2dc43290aabfb171056e47`.

## Layout
- `solve.py <evidence_dir>`  all answers from evidence bytes -> JSON (= day-05 rust-belt solve.py + a d06 block)
- `live_run.sh [start|retry] [time_trial]`  one-process start -> download -> 7z -> solve -> submit (`artifacts/<mode>/run_<ts>/`)
- `mastery_run.sh [start|practice]` + `mastery.py <run_dir>`  deliveries -> answers keyed `setN.<id>` -> submit
- Token: `set -a; . ../../../../.env; set +a` before the shell scripts.
- `artifacts/single_solve/{ev,jar}` = extracted evidence + unzipped jar (javap is not installed; see below).

## Question shape
Same 40 questions as day-05 rust-belt (d01..d05) plus 8 for d06 (48/set): `d06_artifact_sha256_1`, `d06_analysis_format` (`JAR`),
`d06_analysis_config_{campaign,domain,mutex,url}`, `d06_analysis_config_crc32_hex` (0x-prefixed), `d06_analysis_hcfg_version`
(integer, base-10 -> `"1"`). Grading rules from go-fish/rust-belt NOTES all still apply and were accepted unchanged.

## The new thing: HCFG inside a Java class constant pool
`d06--variant-NNN.jar` (plain unencrypted zip: META-INF/MANIFEST.MF, `Day06VariantNNN.class`, `Day06VariantNNN$Result.class`).
The main class has `static final String CONFIG = "<base64>"` (ConstantValue -> a CONSTANT_Utf8 in the pool); `decodeConfig()`
base64-decodes it and the program searches `"campaign":"` in the XOR-decoded JSON (host_model switch is decoy dispatch logic).
The base64 decodes to the standard blob: `HCFG | ver=01 | xor key | len LE16 | crc32 LE32 | payload^key` (key 0x71 in variant-000).
`solve.py::jar_hcfg` walks every class's constant pool (own ~15 line parser, no javap), tries each Utf8 that looks like base64,
and falls back to scanning raw/base64 runs in every jar member. `hcfg()` takes `with_ver=True` to also return the version byte.
`d06_analysis_hcfg_version` is that header byte (1), NOT the class-file major version (0x41 = Java 21) -> do not confuse.

## Mode behaviour (same as go-fish / rust-belt)
- time_trial: 7 s deadline, 256 pool; one-process `live_run.sh start time_trial` (~5.6 s wall incl. 7z) won first try. HoF eligible.
- mastery: 7 s deadline, `unique_attempts_remaining: 0`, not HoF eligible; `mastery_run.sh start` won within the timer (~8 s total
  wall, deadline fine). Fallback if ever needed: `mastery_run.sh practice`.
- Variant mix seen: time_trial = d03 variant-001, d04 variant-006, d05 variant-007, d06 variant-002.jar (== mastery set1 mix).
  mastery set2 = d02 `.exe`, d03 variant-000, d04 `.exe` (Go PE), d05 variant-002 ELF, d06 variant-007.jar. solve.py handled all unchanged.
- Not yet seen: d01 ELF32/big-endian, d05 as PE, d06 as something other than a jar (solve.py would report format 'unknown').
