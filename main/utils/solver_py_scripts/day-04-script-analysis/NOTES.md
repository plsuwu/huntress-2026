# Day 4 "Dim and Dimmer" - status notes (2026-10-14)

Challenge: `script-analysis-day-04-shell-contrast` (231 answers: 3 PowerShell, 9 JScript, 9 Batch, 9 VBScript specimens).
Modes: single_solve, `/time_trial` (6 s deadline, 32 ranked variants), `/mastery` (7 s, 2 sets, 32 variants).

## Files
- `live_run.py` - Day 4 timed runner (`time_trial|mastery|single_solve --op start|retry|practice`); warms TLS before
  the timer, writes start JSON + download to `fixtures/run-*/` before solving.
- `tests/test_mastery_routing.py` - offline outer-zip/`setN.` routing test built from two saved fixtures (needs `uv run --with pyzipper`).
- `tests/test_vbs_mutations.py` - 25 mutation cases (every transform family x every control wrapper x decoys).
- `dim_and_dimmer.py` - driver (CLI + `solve()`); imports the Day 3 solver unchanged for PS/JS/Batch and
  adds VBScript routing. `--zip` or `--dir`, `--questions start.json`, `--out`, `--log`.
- `vbs_static.py` - static VBScript analyser (tiny interpreter for the `Load_`/`Decode_` subset + lexical control-flow
  classifier). No WSH, no eval of artifact code.
- `fixtures/single-solve/` - `start.json`, `evidence.zip` (AES, password `infected`), `extracted/`, `answers.json`
  (as submitted), `evidence-log.json` (per-VBS-specimen descriptor/representation/transform/control), `submit-result.json`.

Run: `python3 -I dim_and_dimmer.py --dir fixtures/single-solve/extracted --questions fixtures/single-solve/start.json`
(`--zip` needs `pyzipper` for AES; the archive was extracted with `7z x -pinfected`).

## Result
| Mode | Result |
|---|---|
| Single solve | CORRECT on the first full submit (231/231); flag in `fixtures/single-solve/submit-result.json` |
| Time trial | CORRECT on the 2nd timed attempt (`retry`; 30 of 32 unique attempts left; Hall-of-Fame eligible). Server stamped the solve ~3.1 s after `started_at` of a 6 s window; client-observed wall time was 7.4 s because each round trip costs 0.8-2.5 s |
| Mastery | CORRECT on the 1st attempt (`start`; 462/462, 31 unique attempts left, Hall-of-Fame eligible). Server stamped the solve ~2.6 s into a 7 s window (deadline 12:33:55.287); client wall time 11.3 s (start 5.6 s for a 316 KB body, download 1.6 s, submit 3.9 s) |

## What Day 4 is
Day 3's archive (PS + JS + Batch) with the **same question ids and byte-identical question text** for every
shared field (diffed against Day 3 time-trial fixture: 0 differences), plus 9 VBScript specimens
(`vbscript-<role>-{role,attack,adapter,campaign,<value>,representation,transform,control}`, 8 per specimen = 72 fields).
Day 3's empirical Batch rules (see `../day-03-script-analysis/NOTES.md`) transferred unchanged and were accepted.

## VBScript specimen anatomy
```
Const Material_<id> = "<encoded descriptor>"        ' same pipe descriptor as Batch: v=1|steps=...|adapter=...|...
Const Codec_<id>    = "<representation>|<family>|<key>"
Function Load_<id>()    ' string -> byte array
Function Decode_<id>(x) ' byte transform (named <id> suffix; variable names are randomised/mixed-case per specimen)
Function Recover_<id>() ' wrapper + Decode call + Chr loop
Function Inspect_<id>() ' Split/Join or Copy sub; no-op
```
- **Descriptor**: evaluate `Load_`+`Decode_` with the mini interpreter, `Chr`-join, parse with Day 3's
  `parse_pipe_descriptor`. Role / ATT&CK reuse `role_of` / `attack_of` with language technique `T1059.005`.
- **representation** answer = `Codec_` field 0 (`decimal_pipe`, `decimal_dash`, `reversed_decimal`, `hex_compact`,
  `hex_colon`); cross-checked against `Load_` structure (`Split` delimiter, `&H`, `Mid`, reversed write index).
- **transform** answer = `Codec_` field 1. Derived independently by probing the decoder with a ramp vector; note the
  label names the *encoder*: a decoder that **adds** N is `subtract_modulo_256`, one that subtracts is
  `add_modulo_256`. `rolling_xor` seeds `previous` with the key byte; `repeating_key_xor` keys are recovered from the
  zero-vector probe.
- **control** = innermost still-open `For`/`Do`/`While`/`Select Case` enclosing the `Decode_<id>(` call in `Recover_`;
  else `On Error Resume Next` active -> `error_guard`; else `direct`. Decoys that must NOT count: `If True Then ... End If`
  wrapping the real wrapper (host-context, outbound-transfer, staged-handoff, staged variants), single-line
  `If (2 > 1) Then x = .. Else x = ..`, `LCase(UCase("Host"))`, `UBound(Array(...))`, arithmetic noise, and `Case Else`
  `Array()` branches. No `If` option exists in the VBS control enum.
- Results seen: benign/dropper/relay -> `select_case`; downloader/uploader/chain -> `do_once`; impact -> `while_once`;
  stager/launcher -> `direct`. `for_once` and `error_guard` were not present in this variant (supported in code
  but untested against the grader).
- Relay specimens again have steps `acquire_remote,write_file,dispatch_process,handoff_script` -> `staged_chain`
  (`T1059.005`), same as the Day 3 graded relay rule.

## Time trial log
- **Attempt 1 (burned):** `pyzipper` was not importable under plain `python3` (this repo's `python3` has no AES zip support),
  so the runner died after the download. Fixture kept in `fixtures/time-trial-1-burned/`. **Always run the live runner via
  `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt uv run --with pyzipper python3 live_run.py ...`** - and do `--dry`
  against a saved fixture first. (`SSL_CERT_FILE=` set to an empty string breaks TLS too.)
- **Attempt 2:** variant 2 (`fixtures/time-trial-2/`), 231/231, CORRECT.
- Variant 2 differed from single-solve in VBScript only, and needed one solver change (made *after* seeing it, from the
  burned attempt's fixture): an in-file `Base64_<id>` helper (standard 6-bit accumulator; alphabet literal read from the
  helper; Load_ glues `Mid(...)` chunks with `&`). `Load_`/`Decode_` bodies may therefore contain the helper call, but
  `If`/`Select`/`Do` inside the helper are never interpreted - the helper is replaced by a native decoder.
- New values seen in variant 2: representation `base64_chunks`, `base64_literal`, `hex_dash`, `hex_colon`; transform
  `identity`, `reverse_then_xor`; control `error_guard` (`On Error Resume Next` directly before the Decode call, then
  `e = Err.Number ... On Error GoTo 0 ... If e <> 0 Then Err.Raise`). All accepted.
- The add/subtract family is ambiguous behaviourally (+k == -(256-k)); disambiguated by the decoder's own operator
  (`+` in decoder => `subtract_modulo_256`, `-` => `add_modulo_256`), verified against the label in single-solve only
  for the `+` case.
- Latency: tiny GET 0.8-1.3 s, `start` 3.7 s (154 KB body; gzip does not help - server-side time), download 1.3 s,
  submit 2.4 s. Solver CPU ~0.1 s. The server's receipt timestamp is what counts, not the client's wall clock.

## Risks / open items (all three modes are CORRECT)
- Only one variant has been seen. New VBS variants may bring `for_once` / `error_guard`, `Do While`/`Do Until`
  loops, `Step`/`Exit For`, `If` conditions inside decoder bodies (the interpreter raises `VbsUnsupported` on `If`,
  `Select`, `Do`, `Call` inside `Load_`/`Decode_`; Recover is analysed lexically only), or a `Codec_` label that
  disagrees with the code (`NOTE` lines are printed and recorded in `--log`; none in this variant).
- Mastery (done, `fixtures/mastery-1/`): one download (61 KB outer zip) holding evidence-01/02.zip, ids `set1.<id>`/`set2.<id>`,
  routed by `ep.pick_delivery` on the `artifact` label; solved 462/462 with no solver changes. The two sets were not
  inspected for new VBS families beyond what is logged in `evidence-log` - rerun `dim_and_dimmer.py --zip` on the fixture to see.
- Before burning a timed attempt, mutate the fixture (rename identifiers, swap wrapper/transform, add `On Error Resume
  Next`, stack a `For` inside `If True`) and assert answer counts.
- Rust port (`ctf-run`) not started; port from this Python only after time trial / mastery are CORRECT.
- Optional: re-run mastery (31 attempts left) / time trial (30 left) only for a faster Hall-of-Fame time; the server's
  receipt timestamp counts and the wall time is dominated by the 316 KB `start` response.
