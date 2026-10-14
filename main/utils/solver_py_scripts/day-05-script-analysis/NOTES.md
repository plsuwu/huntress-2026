# Day 5 "Hatch Careful" - status notes (2026-10-14)

Challenge: `script-analysis-day-05-python-carrier-lab` (357 answers = Day 4's 231 + **126 new Python** answers:
14 specimens x 9 fields). Modes: single_solve, `/time_trial` (7 s, 32 ranked variants), `/mastery` (7 s, 2 sets = 714 ids
`set1.<id>`/`set2.<id>`, 2 deliveries). All three **CORRECT**.

## Files
- `hatch_careful.py` - driver (`--zip|--dir`, `--questions start.json`, `--out`, `--log`). Delegates every non-`python-`
  question to Day 4's `dim_and_dimmer.solve` (PS/JS/Batch/VBS, imported unchanged) and answers the Python ones itself.
  `python-` routing must strip the mastery `setN.` prefix (`_is_python_question`).
- `py_static.py` - ast-based, whitelisted mini interpreter. **Nothing is exec'd/imported/compiled**: `ast.parse` + a small
  evaluator (assign/def/lambda/closure/class+@staticmethod/decorators/if/while/for/try/match/comprehensions/f-strings/
  walrus, str/bytes/list/dict methods, base64/binascii/io/os.environ.get->default). Unknown construct => `PyUnsupported`.
  It runs `inspect_<id>()`, `surface_<id>()`, `obfuscation_<id>()` and `plan_<id>()` of each specimen.
- `live_run.py` - Day 4 runner adapted (`time_trial|mastery|single_solve --op start|retry|practice`). Run with
  `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py ...`; token via env.
- `tests/test_regression.py` - re-solves the saved single-solve / time-trial-1 / mastery-2 fixtures offline and
  compares with what the grader accepted (357/357/714).
- `fixtures/` - `single-solve/` (start, AES `evidence.zip`, `extracted/`, answers, evidence-log, submit-result),
  `time-trial-1/`, `mastery-2/` (two zips), `mastery-1-burned/` (see below).

## Python specimen anatomy
```
MATERIAL_<id>  = <literal>                       # str / bytes / int list / implicitly-concatenated str chunks
CODEC_<id>     = "<representation>|<transform>|<key>"
load_<id>() decode_<id>(x) recover_<id>() inspect_<id>()     # bytes -> descriptor text (ascii)
surface_<id>()      -> 4-tuple (syntax, obfuscation, carrier, windows_chain)
obfuscation_<id>()  -> must equal surface[1]
plan_<id>()         -> (adapter-ish, ...values) + (surface, obfuscation)
if os.environ.get("HUNTRESS_INSPECT_ONLY") == "1": print(...)    # never true here
```
- Descriptor is the same pipe format as Batch/VBS but `v=2` and it **also repeats** `syntax|obfuscation|carrier|windows_chain`
  (all four matched surface()/obfuscation() in every fixture; mismatches are flagged in `notes`).
- Answers: role/attack from `steps` (Day 3 `role_of`/`attack_of`, language technique **T1059.006**); adapter =
  descriptor `adapter`; campaign = `campaign_id`; `-file-path/-url/-upload-url/-next-stage/-extension/-host-id/-handoff-id`
  = descriptor key with hyphens->underscores; syntax/obfuscation/carrier/windows-chain = surface()[0..3].
- Surface vocab seen: syntax {class_staticmethod, closure_factory, decorator_wrapper, dictionary_dispatch,
  exception_expression, generator_pipeline, iterator_protocol, lambda_dispatch, list_comprehension, match_case,
  plain_functions, walrus_binding}; carrier {raw_source, hash_based_pyc_38, hash_based_pyc_current, zipapp_pyz,
  wheel_archive, pyinstaller_onefile/onedir, py2exe_console, py2exe_windows_service, cx_freeze_executable, cx_freeze_msi,
  pyarmor_obfuscated, pyarmor_pyinstaller, embeddable_python_runtime}; windows-chain {direct_python_exe, renamed_interpreter,
  cmd_to_python, powershell_to_python, mshta/wscript/rundll32/regsvr32_*, pythonw_hidden_console, service/scheduled_task/run_key_descriptor}.
- Roles seen: benign_utility (source), dropper (pyc38, cxfreeze-msi), launcher (current-pyc, py2exe-console), relay
  (zipapp, embedded-runtime), stager (wheel, pyarmor-source), downloader (onefile), uploader (onedir),
  staged_chain (py2exe-service, pyarmor-pyinstaller: profile_host,acquire_remote,decode_embedded,write_file,dispatch_process),
  impact (cxfreeze-executable -> T1486). The **filename/carrier name does not decide the role** (service/msi/executable
  names are decoys); only the decoded `steps` do.
- `source-context.py` benign: `plan[0]` is the constant `report`, the descriptor `adapter` is `argparse_namespace`
  (mastery: `dataclass_report`, `mapping_proxy`). The descriptor adapter was accepted.
- `invoke-python-stage.ps1` (extra inert PS file) just carries `$HandoffId`/`$NextStage`; the same `handoff_id` is in the
  pyarmor-pyinstaller descriptor, so no question needed the file. `analyse_ps_handoff` exists in `py_static.py` but is
  unused by the driver.

## Results / timing
| Mode | Result |
|---|---|
| Single solve | CORRECT first submit, 357/357 |
| Time trial | CORRECT on the 1st attempt (`start`), 31 unique attempts left, Hall-of-Fame eligible. Server-stamped solve 4.4 s into the 7 s window; client wall time 9.2 s (start 4.0 s for a 232 KB body, download 1.5 s, submit 3.6 s) |
| Mastery | attempt 1 **burned** (below); attempt 2 (`retry`) CORRECT 714/714, 30 attempts left, HoF eligible. Server stamped the solve only **0.27 s** before the 7 s deadline - client wall 12.3 s (retry 4.7 s for a 484 KB body, 2 downloads 3.2 s, submit 4.2 s) |

## Lessons / risks
- **Burned mastery attempt 1:** the router checked `qid.startswith("python-")`, but mastery ids are `set1.python-...`, so the
  252 Python answers were routed to the Day 4 solver and errored; the submit was `accepted`/`remaining: 252` and the timer
  then expired (`result: expired`; retry restarts a fresh one). Fixture in `fixtures/mastery-1-burned/`. Routing is now
  covered by `tests/test_regression.py`. **Always dry-run a new-prefix change against a saved mastery fixture before `start`.**
- Mastery is tight: ~0.3 s margin. The server stamps the receipt time; the bottleneck is the 484 KB start body + 2 downloads
  + the 252-answer submit. If a re-run for Hall of Fame is wanted, consider reusing one TLS connection (already done) and
  skipping the pyzipper `uv run` start-up (use a venv with pyzipper preinstalled; `uv run --with` adds ~0.3 s *before* the
  timer starts, so it does not matter) - only post-`start` latency counts.
- Only three variants seen (single, TT-1, mastery-2 x2 sets). Python variants so far used only the constructs in the
  interpreter; a new construct raises `PyUnsupported` (visible as `ERR` lines - the runner still submits whatever was solved,
  so a partial submit is possible but pointless on a timer).
- Rust port (`ctf-run`) not started for Day 5.
