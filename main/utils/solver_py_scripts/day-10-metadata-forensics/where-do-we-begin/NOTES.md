# Day 10 metadata-forensics - Where Do We Begin? (`metadata-forensics-day-10-where-do-we-begin`)

PE binaries in `metadata-day-10-evidence.zip`; every question in every mode was one kind:
`d09-pe-NNN-pe-address-of-entry-point-rva-hex` ("Determine pe.address_of_entry_point_rva_hex for <name>.exe").
Answer = Optional Header AddressOfEntryPoint formatted **`0x%08x`** (zero-padded 8 hex digits, e.g. `0x00065bd0`);
the format was learned on day 13 (`0x65bd0` is rejected). The "report that is lying" hint never surfaced as a question.

`solve.py` = the day-11/12 solver (itself the day-13 solver) with `CHAL` swapped; `pe_entry_rva` handler unchanged.
`msi_meta.py` is unused here but is a dependency of solve.py.

## Results (2026-10-14) - all first try
| mode | answers | flag |
|---|---|---|
| single_solve | 1 (`0x00065bd0`) | `b994664eb68f6a890a67df889a1b7992` |
| time_trial (ranked, 8 s, pool 2 -> 1 left) | 1 (`0x00063a80`) | `766f275a825f98cb5712652b6641c6cf` |
| mastery (ranked, 11 s, 2 sets) | 2 (`set1.` `0x00063510`, `set2.` `0x00065bd0`) | `a78522058a7c591264443439b8b7e9e5` |

- Route `run_for_time_supported:false` -> time_trial/mastery timed by default; one of two ranked attempts used per mode.
- Run: `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} --submit [--force]`.
- Fixtures: `artifacts/` (start JSONs, latest zips).
