# Routine Check (day 07, binary-reverse-engineering-day-07-kotlin-staging-logic)

Status: single_solve, time_trial, mastery all CORRECT first try (2026-10-14). Static analysis only; nothing delivered was executed.
Flags: single `6a8bfc0c132c08251c9d8c6b724e2049`, time_trial `aa596ef490b20163d07ace3ae570a36e`, mastery `2977d09e332848a28000e07846ed59cf`.

## Layout
- `solve.py <evidence_dir>`  all answers from evidence bytes -> JSON (= day-06 pool-party solve.py, d06/d07 handled by one loop)
- `live_run.sh [start|retry] [time_trial]`  one-process start -> download -> 7z -> solve -> submit (`artifacts/<mode>/run_<ts>/`)
- `mastery_run.sh [start|practice]` + `mastery.py <run_dir>`  deliveries -> answers keyed `setN.<id>` -> submit
- Token: `set -a; . ../../../../.env; set +a` before the shell scripts (run from the repo root or any dir; paths are script-relative).
- `artifacts/single_solve/{ev,jar}` = extracted evidence + the d07 Day07Variant000 app classes (javap/java not installed).

## Question shape
56 per set = day-06's 48 + 8 for d07, which are the d06 JAR questions verbatim: `d07_artifact_sha256_1`, `d07_analysis_format` (`JAR`),
`d07_analysis_config_{campaign,domain,mutex,url}`, `d07_analysis_config_crc32_hex` (0x-prefixed), `d07_analysis_hcfg_version` (`"1"`).
Rules from go-fish / rust-belt / pool-party NOTES all apply unchanged.

## The new thing: a Kotlin fat jar (and nothing else)
`d07--variant-NNN.jar` is ~5 MB / 3267 members because the Kotlin stdlib is bundled. App classes: `Day07VariantNNN.class`,
`Day07VariantNNN$staged$1.class` (the coroutine `ContinuationImpl` state machine, `label` field + `invokeSuspend`),
`$main$1/$main$2`, `Result.class`. The coroutine/state-machine machinery (suspendCoroutine -> `seed`, `host_model` switch
sandbox/workstation/analysis -> `Result(branch, events)`, the `huntress.binary.dynamic_trace.v1` JSON) is decoy dispatch logic: the
HCFG blob is the same `static final String CONFIG` base64 constant as d06 (`HCFG|ver|key|len LE16|crc32 LE32|payload^key`).
`decodeConfig()` base64-decodes it and the program searches `"campaign":"`. No state-machine reconstruction is needed for any answer.
Only change to solve.py: `jar_hcfg` sorts constant pools so app classes are parsed before `kotlin/`, `kotlinx/`, `org/`, `META-INF/`
(0.05 s total either way; matters only for the 7-12 s timers). `d07_analysis_hcfg_version` is the HCFG header byte (1), not the
class-file major version.

## Mode behaviour
- time_trial: deadline 11 s, 256 pool; `live_run.sh start time_trial` ~7 s wall incl. 7z; won first try; HoF eligible.
- mastery: deadline 12 s, `ranked_pool_size` 1 / `unique_attempts_remaining: 0`, not HoF eligible; `mastery_run.sh start` won in ~10.6 s
  wall (close to the deadline - 7z of the nested zips dominates; if it ever times out use `mastery_run.sh practice`).
- Variant mix seen: single d03 v000/d04 v000 ELF/d05 v000/d06 v000/d07 v000. time_trial = d03 v001, d04 v006, d05 v007, d06 v002, d07 v005
  (== mastery set1). mastery set2 = d02 .exe, d03 v000, d04 v002 .exe, d05 v002, d06 v007, d07 v004. solve.py handled all unchanged.
- Not yet seen: d01 ELF32/big-endian, d07 as anything other than a jar with the CONFIG constant (solve.py would report format 'unknown'
  and the jar_hcfg fallback scans raw/base64 runs in every member).
