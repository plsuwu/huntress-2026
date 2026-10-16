# Day 16 binary RE - WebAssembly Optimization Equivalence ("Wasm Up")

d16 ships two builds of one tiny wasm module (~560 B each): `d16--variant-NNN-compact.wasm` (select-chain
`evaluate`) and `d16--variant-NNN-direct.wasm` (nested if/else). Exports `memory` + `evaluate`; the HCFG blob
(raw, validated by CRC) sits in a data segment at linear address 0x400, followed by the strings
`huntress.binary.dynamic_trace.v1` / `sandboxed_module_inventory` (decoration - never asked).

Questions (10 for d16, 160 total): `d16_artifact_sha256_{1,2}` (zip CD order: compact first),
`d16_analysis_{compact,direct}_{architecture,format}`, `d16_analysis_compact_config_{campaign,domain,mutex,url}`.

Graded (single-solve CORRECT first submit):
- `format` (exact_text) = `wasm`; `architecture` = `wasm32` (memory64 bit clear; `formats/wasm.rs` would say `wasm64`).
- Config = HCFG as always.

Change: new `ctf-run/src/formats/wasm.rs` (section walk: custom names, imports, exports, memory flags) +
`Container::Wasm` in `lanes/binary_re.rs`. The prompt ("imports, memory, indirect calls") suggests later days may ask
about wasm imports/exports/tables - the parser already collects imports/exports; add `answer_spec` rows when asked.

Time trial / mastery: untimed regular attempts run by the coordinator with the lane build (unchanged code):
time trial CORRECT first submit (160 answers), mastery CORRECT first submit (800 answers, 5 sets). Ranked pools:
TT 64, mastery 16 (both dual-pool, rft=true). The lane agent's plain `-t` start was blocked by its permission
classifier - lane agents may need the coordinator for untimed TT/mastery starts.
Fixtures: `ctf-run/fixtures/binary_re/d16-{single_solve,time_trial,mastery}.json`.
