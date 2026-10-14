# Day 12 metadata-forensics - Managed Expectations (`metadata-forensics-day-12-managed-expectations`)
.NET assemblies in `metadata-day-12-evidence.zip`; every question in every mode was a single kind:
`dN-dotnet-NNN-dotnet-mvid-guid` ("Determine dotnet.mvid_guid for managed-NNN.dll"). The prompt's "rebuilds vs renames"
hint never surfaced as a grouping question - one MVID per question, per-assembly.

- Answer = Module-table Mvid via the `#GUID` heap, **lowercase bare 8-4-4-4-12** (`dotnet_mvid` in `solve.py`, unchanged
  from day-13; it is the same handler validated on the day-13 TT bundle).
- `solve.py` is the day-13 solver with the challenge id swapped. Added flags: `--force` (submit partial answers even if a
  kind is unhandled - needed on pool-1 ranked routes) and `--practice` (op=practice for the untimed continuation).

## Results (all first try, no burned attempts)
| mode | result | flag |
|---|---|---|
| single_solve | CORRECT | a80712d07b49eb38d3e1e44ce469f2fe |
| time_trial (ranked, 30 s, 1 set) | CORRECT | 71f475ffb3e90854c798b5710beafed8 |
| mastery (ranked, 30 s, 2 sets, ids `set1.`/`set2.`) | CORRECT | 2de98a55ea933cac371546792d468742 |

- Route `run_for_time_supported: false`: ranked pool is 1 (`unique_attempts_remaining` 0 after the first start), so the
  timed attempts are consumed; both completed in well under 1 s of the 30 s budget.
- Run: `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} --submit [--force]`.
- Fixtures: `artifacts/` (start JSONs, latest zips).
