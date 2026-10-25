# Day 25 metadata - Hive Mind (`metadata-forensics-day-25-hive-mind`)

Prompt: "Registry hives from multiple systems have been renamed and separated from host labels. Identify each true
root, regroup the systems, and isolate the planted hive." As usual only one new question kind came out of it:
single-solve asks it once, TT is the cumulative bundle (30 q), and mastery has 9 sets (270 q).

## New kind: `d25-registry-001-registry-hive-root-name`
- Graded: the name of the `nk` cell that the base block's root-cell offset (u32 @0x24, relative to 0x1000) points
  at. That is the "true root". It is **not** the base block's embedded UTF-16 file name @0x30, which is only the
  path tail and differs from the root name: `ress-artifact-assembly-line.hiv` vs root `HuntressArtifactAssemblyLine`
  in single-solve, and `juniper-1af548c5.hiv` vs `JPkQQe9xqgn3` in TT. Root names are case-sensitive random
  strings in TT/mastery.
- The hives are small (8192 B: a base block plus one hbin). The root has flags 0x2c (HIVE_ENTRY|NO_DELETE|COMP_NAME),
  no subkeys, and values `Marker` (REG_SZ `HUNTRESS-HIVE-NNNN`) and `Ordinal` (DWORD). Each surveyed hive has exactly
  one HIVE_ENTRY `nk`, so there was no decoy root.
- New `ctf-run/src/formats/regf.rs`: base block (sequence numbers, XOR checksum, file name), hbin walk, nk/vk/
  lf/lh/li/ri, a tree walk and `value_text`. `lanes/metadata.rs::registry_answer` dispatches on `-registry-<field>`
  before HANDLERS. It refuses when the root cell is not an `nk` with KEY_HIVE_ENTRY. Only `hive-root-name` has been
  graded. The other fields (file-name, sequences/dirty, checksum, timestamps, key/value counts, root subkey/value
  names, any root value by name such as `marker`) are guesses. An unknown field fails with a `registry_facts` dump.
- Predicted follow-ups: "regroup the systems" (the Marker value or a host-label value) and "planted hive" (sequence
  mismatch, bad checksum or timestamp outlier). Those fields are already answered as ungraded guesses.

## Runs (all untimed, all CORRECT on the first submit)
- `runs-d25/.../single_solve-9548045d-1792933393`, `time_trial-8dfe69b9-1792933406`, `mastery-5c207df6-1792933422`
  -> `fixtures/metadata/d25-{single_solve,time_trial,mastery}.json` (replay 67/67 clean).
