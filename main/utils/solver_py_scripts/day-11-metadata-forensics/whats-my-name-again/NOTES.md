# Day 11 metadata-forensics - What's My Name Again? (`metadata-forensics-day-11-whats-my-name-again`)

PE binaries with randomised member names in `metadata-day-11-evidence.zip`; every question in every mode was one kind:
`d09-pe-NNN-pe-version-original-filename` ("Determine pe.version_original_filename for <name>.exe").
Answer = VS_VERSION_INFO StringFileInfo `OriginalFilename` (UTF-16LE), e.g. `pe-base-386.exe` - NOT the zip member name.
The "impersonator" hint never surfaced as a separate question.

`solve.py` = the day-12 `managed-expectations` solver (itself the day-13 solver) with `CHAL` swapped; `pe_original_filename`
was already validated on the day-13 time-trial bundle. `msi_meta.py` is unused here but kept as a dependency of solve.py.

## Results (2026-10-14) - all first try
| mode | answers | flag |
|---|---|---|
| single_solve | 1 (`pe-base-386.exe`) | `76a04f5e3e99d4b87891acb0f65ea31d` |
| time_trial (ranked, 7 s, pool 2 -> 1 left) | 1 (`pe-base-amd64.exe`) | `e16d5a5e967771ad30c2436d7ec73e3f` |
| mastery (ranked, 7 s, 2 sets) | 2 (`set1.`/`set2.`: arm64, 386) | `b45d5e840de357ab57e400d42d9256a3` |

- Route `run_for_time_supported:false` -> time_trial/mastery are timed by default (7 s deadline); pools are 2 ranked
  attempts each; one was used per mode, 1 remains (use `--retry` if a re-run is ever wanted).
- Run: `set -a; . ./.env; set +a; python3 -I solve.py {single_solve|time_trial|mastery} --submit [--force]`.
- Fixtures: `artifacts/` (start JSONs, latest zips).
